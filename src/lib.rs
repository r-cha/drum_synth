use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;
use rand::Rng;
use std::sync::Arc;
use std::f32::consts::PI;

mod ui;


/// The maximum size of a delay buffer for resonance.
/// ~100ms at 44.1kHz sample rate
const MAX_DELAY: usize = 4096;

struct OnePoleFilter {
    a0: f32,
    b1: f32,
    z1: f32,
}

impl OnePoleFilter {
    fn new() -> Self {
        Self {
            a0: 1.0,
            b1: 0.0,
            z1: 0.0,
        }
    }
    
    /// Set filter coefficients for low/high pass
    /// cutoff should be 0.0-1.0 (normalized frequency)
    fn set_cutoff(&mut self, cutoff: f32, lowpass: bool) {
        let g = (PI * cutoff).tan();
        let a1 = if lowpass { (g - 1.0) / (g + 1.0) } else { (1.0 - g) / (1.0 + g) };
        self.a0 = (1.0 + a1) / 2.0;
        self.b1 = a1;
    }
    
    /// Process one sample through the filter
    fn process(&mut self, input: f32) -> f32 {
        let output = self.a0 * input + self.a0 * self.z1;
        self.z1 = input - output * self.b1;
        output
    }
    
    /// Reset filter state
    fn reset(&mut self) {
        self.z1 = 0.0;
    }
}

/// Simple peak EQ
struct PeakEQ {
    a0: f32,
    a1: f32,
    a2: f32,
    b1: f32,
    b2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl PeakEQ {
    fn new() -> Self {
        Self {
            a0: 1.0,
            a1: 0.0,
            a2: 0.0,
            b1: 0.0,
            b2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }
    
    /// Configure peak EQ
    /// freq: 0.0-1.0 (normalized frequency)
    /// gain: gain in dB
    /// q: q factor (bandwidth)
    fn configure(&mut self, freq: f32, gain: f32, q: f32, sample_rate: f32) {
        let omega = 2.0 * PI * freq / sample_rate;
        let alpha = (omega.sin()) / (2.0 * q);
        let a = 10.0_f32.powf(gain / 40.0);
        
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * omega.cos();
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * omega.cos();
        let a2 = 1.0 - alpha / a;
        
        self.a0 = b0 / a0;
        self.a1 = b1 / a0;
        self.a2 = b2 / a0;
        self.b1 = a1 / a0;
        self.b2 = a2 / a0;
    }
    
    /// Process one sample through the EQ
    fn process(&mut self, input: f32) -> f32 {
        let output = self.a0 * input + self.a1 * self.x1 + self.a2 * self.x2
                   - self.b1 * self.y1 - self.b2 * self.y2;
        
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        
        output
    }
    
    /// Reset filter state
    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

pub struct DrumSynth {
    params: Arc<DrumSynthParams>,
    sample_rate: f32,

    // Transient layer (noise burst)
    transient_envelope: ADSREnvelope,
    transient_eq: PeakEQ,

    // Resonance layer (Karplus-Strong)
    resonance_buffer: Vec<f32>,
    resonance_write_pos: usize,
    resonance_read_pos: usize,
    resonance_lowpass: OnePoleFilter, // Damping filter in feedback loop
    resonance_eq: PeakEQ,

    // Snare noise (fed through resonator)
    noise_envelope: ADSREnvelope,
    snare_eq: PeakEQ,
    
    // MIDI tracking
    midi_note_id: u8,
    midi_note_freq: f32,
    is_playing: bool,
    
    // VIZIA editor state
    editor_state: Arc<ViziaState>,
}

struct ADSREnvelope {
    state: ADSRState,
    attack_time: f32,
    decay_time: f32,
    sustain_level: f32,
    release_time: f32,
    hold_time: f32,
    current_level: f32,
    sample_rate: f32,
    hold_samples_left: usize,
}

#[derive(PartialEq)]
enum ADSRState {
    Idle,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
}

impl ADSREnvelope {
    fn new(sample_rate: f32) -> Self {
        Self {
            state: ADSRState::Idle,
            attack_time: 0.01,
            decay_time: 0.1,
            sustain_level: 0.5,
            release_time: 0.1,
            hold_time: 0.0,
            current_level: 0.0,
            sample_rate,
            hold_samples_left: 0,
        }
    }

    fn set_parameters(&mut self, attack: f32, decay: f32, sustain: f32, release: f32, hold: f32) {
        self.attack_time = attack;
        self.decay_time = decay;
        self.sustain_level = sustain;
        self.release_time = release;
        self.hold_time = hold;
    }

    fn note_on(&mut self) {
        self.state = ADSRState::Attack;
        self.current_level = 0.0;
        self.hold_samples_left = (self.hold_time * self.sample_rate) as usize;
    }

    fn note_off(&mut self) {
        if self.state != ADSRState::Idle {
            self.state = ADSRState::Release;
        }
    }

    fn process(&mut self) -> f32 {
        match self.state {
            ADSRState::Idle => 0.0,
            ADSRState::Attack => {
                // Fast attack for percussive sounds
                self.current_level += 1.0 / (self.attack_time * self.sample_rate).max(1.0);
                if self.current_level >= 1.0 {
                    self.current_level = 1.0;
                    if self.hold_time > 0.0 {
                        self.state = ADSRState::Hold;
                    } else {
                        self.state = ADSRState::Decay;
                    }
                }
                self.current_level
            }
            ADSRState::Hold => {
                if self.hold_samples_left > 0 {
                    self.hold_samples_left -= 1;
                    1.0 // Hold at maximum level
                } else {
                    self.state = ADSRState::Decay;
                    1.0
                }
            }
            ADSRState::Decay => {
                self.current_level -= (1.0 - self.sustain_level) / (self.decay_time * self.sample_rate).max(1.0);
                if self.current_level <= self.sustain_level {
                    self.current_level = self.sustain_level;
                    self.state = ADSRState::Sustain;
                }
                self.current_level
            }
            ADSRState::Sustain => self.sustain_level,
            ADSRState::Release => {
                self.current_level -= self.current_level / (self.release_time * self.sample_rate).max(1.0);
                if self.current_level <= 0.001 {
                    self.current_level = 0.0;
                    self.state = ADSRState::Idle;
                }
                self.current_level
            }
        }
    }

    fn is_active(&self) -> bool {
        !matches!(self.state, ADSRState::Idle)
    }
}

#[derive(Params)]
pub struct DrumSynthParams {
    #[id = "gain"]
    pub gain: FloatParam,

    // Impact layer params (transient)
    #[nested(group = "Impact")]
    impact_params: ImpactParams,
    
    // Tuning layer params (resonance)
    #[nested(group = "Tuning")]
    tuning_params: TuningParams,

    // Snare layer params
    #[nested(group = "Snare")]
    snare_params: SnareParams,
}

#[derive(Params)] 
struct ImpactParams {
    #[id = "tr_attack"]
    pub attack: FloatParam,
    
    #[id = "tr_hold"]
    pub hold: FloatParam,
    
    #[id = "tr_decay"]
    pub decay: FloatParam,
    
    #[id = "tr_release"]
    pub release: FloatParam,
    
    #[id = "tr_level"]
    pub level: FloatParam,
    
    #[id = "tr_eq_freq"]
    pub eq_freq: FloatParam,
    
    #[id = "tr_eq_gain"]
    pub eq_gain: FloatParam,
    
    #[id = "tr_eq_q"]
    pub eq_q: FloatParam,
}

#[derive(Params)]
struct TuningParams {
    #[id = "res_delay_samples"]
    pub delay_samples: FloatParam,
    
    #[id = "res_feedback"]
    pub feedback: FloatParam,
    
    #[id = "res_damping"]
    pub damping: FloatParam,
    
    #[id = "res_level"]
    pub level: FloatParam,
    
    #[id = "res_eq_freq"]
    pub eq_freq: FloatParam,
    
    #[id = "res_eq_gain"]
    pub eq_gain: FloatParam,
    
    #[id = "res_eq_q"]
    pub eq_q: FloatParam,
}

#[derive(Params)]
struct SnareParams {
    #[id = "snare_attack"]
    pub attack: FloatParam,
    
    #[id = "snare_decay"]
    pub decay: FloatParam,
    
    #[id = "snare_level"] 
    pub level: FloatParam,
    
    #[id = "snare_eq_freq"]
    pub eq_freq: FloatParam,
    
    #[id = "snare_eq_gain"]
    pub eq_gain: FloatParam,
    
    #[id = "snare_eq_q"]
    pub eq_q: FloatParam,
}

impl Default for DrumSynth {
    fn default() -> Self {
        Self {
            params: Arc::new(DrumSynthParams::default()),
            sample_rate: 44100.0,

            transient_envelope: ADSREnvelope::new(44100.0),
            transient_eq: PeakEQ::new(),

            resonance_buffer: vec![0.0; MAX_DELAY],
            resonance_write_pos: 0,
            resonance_read_pos: 0,
            resonance_lowpass: OnePoleFilter::new(),
            resonance_eq: PeakEQ::new(),

            noise_envelope: ADSREnvelope::new(44100.0),
            snare_eq: PeakEQ::new(),
            
            midi_note_id: 0,
            midi_note_freq: 1.0,
            is_playing: false,
            
            editor_state: ViziaState::new(|| (1000, 750)),
        }
    }
}

impl Default for ImpactParams {
    fn default() -> Self {
        Self {
            attack: FloatParam::new(
                "Attack",
                0.0005, // 0.5ms
                FloatRange::Skewed {
                    min: 0.0001,
                    max: 0.01,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" s"),
            
            hold: FloatParam::new(
                "Hold",
                0.0, // 0ms
                FloatRange::Skewed {
                    min: 0.0,
                    max: 0.01,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" s"),
            
            decay: FloatParam::new(
                "Decay",
                0.02, // 20ms
                FloatRange::Skewed {
                    min: 0.01,
                    max: 0.03,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" s"),
            
            release: FloatParam::new(
                "Release",
                0.015, // 15ms
                FloatRange::Skewed {
                    min: 0.01,
                    max: 0.03,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" s"),
            
            level: FloatParam::new(
                "Level",
                0.8,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1.0,
                },
            ),
            
            eq_freq: FloatParam::new(
                "Tone",
                500.0,
                FloatRange::Skewed {
                    min: 100.0,
                    max: 5000.0,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" Hz"),
            
            eq_gain: FloatParam::new(
                "Tone Gain",
                3.0,
                FloatRange::Linear {
                    min: -12.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB"),
            
            eq_q: FloatParam::new(
                "Tone Width",
                1.0,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 10.0,
                    factor: FloatRange::skew_factor(-1.0)
                },
            ),
        }
    }
}

impl Default for TuningParams {
    fn default() -> Self {
        Self {
            delay_samples: FloatParam::new(
                "Tension",
                44.0,  // About 1ms @ 44.1kHz
                FloatRange::Linear {
                    min: 5.0,  // Very tight head
                    max: 200.0, // Very loose head
                },
            )
            .with_unit(" samples")
            .with_step_size(1.0),
            
            feedback: FloatParam::new(
                "Sustain",
                -0.7,
                FloatRange::Linear {
                    min: -0.99, // Long decay
                    max: -0.3,  // Short decay
                },
            ),
            
            damping: FloatParam::new(
                "Damping",
                0.5, // Middle damping
                FloatRange::Linear {
                    min: 0.1,  // Bright (less filtering)
                    max: 0.9,  // Dark (more filtering)
                },
            ),
            
            level: FloatParam::new(
                "Level",
                0.8,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1.0,
                },
            ),
            
            eq_freq: FloatParam::new(
                "Tone",
                800.0,
                FloatRange::Skewed {
                    min: 100.0,
                    max: 5000.0,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" Hz"),
            
            eq_gain: FloatParam::new(
                "Tone Gain",
                0.0,
                FloatRange::Linear {
                    min: -12.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB"),
            
            eq_q: FloatParam::new(
                "Tone Width",
                1.0,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 10.0,
                    factor: FloatRange::skew_factor(-1.0)
                },
            ),
        }
    }
}

impl Default for SnareParams {
    fn default() -> Self {
        Self {
            attack: FloatParam::new(
                "Attack",
                0.001,
                FloatRange::Skewed {
                    min: 0.0001,
                    max: 0.01,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" s"),
            
            decay: FloatParam::new(
                "Decay",
                0.1,
                FloatRange::Skewed {
                    min: 0.01,
                    max: 0.5,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" s"),
            
            level: FloatParam::new(
                "Level",
                0.3,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1.0,
                },
            ),
            
            eq_freq: FloatParam::new(
                "Tone",
                2000.0, // 2kHz
                FloatRange::Skewed {
                    min: 500.0,
                    max: 10000.0,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" Hz"),
            
            eq_gain: FloatParam::new(
                "Tone Gain",
                6.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB"),
            
            eq_q: FloatParam::new(
                "Tone Width",
                1.0,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 5.0,
                    factor: FloatRange::skew_factor(-1.0)
                },
            ),
        }
    }
}

impl Default for DrumSynthParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new(
                "Gain",
                -6.0,
                FloatRange::Linear {
                    min: -30.0,
                    max: 6.0,
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_step_size(0.01)
            .with_unit(" dB"),
            
            impact_params: ImpactParams::default(),
            tuning_params: TuningParams::default(),
            snare_params: SnareParams::default(),
        }
    }
}

impl DrumSynth {
    fn calculate_noise() -> f32 {
        let mut rng = rand::thread_rng();
        rng.gen_range(-1.0..1.0)
    }
    
    fn process_transient(&mut self) -> f32 {
        // Use white noise burst for transient (not square wave)
        let noise = Self::calculate_noise();
        
        // Apply envelope to transient
        let envelope = self.transient_envelope.process();
        
        // Configure transient EQ
        self.transient_eq.configure(
            self.params.impact_params.eq_freq.smoothed.next(), 
            self.params.impact_params.eq_gain.smoothed.next(),
            self.params.impact_params.eq_q.smoothed.next(),
            self.sample_rate
        );
        
        // Apply EQ and level control
        let output = noise * envelope * self.params.impact_params.level.smoothed.next();
        self.transient_eq.process(output)
    }
    
    fn process_snare_input(&mut self) -> f32 {
        // Generate noise for snare wires
        let noise = Self::calculate_noise();
        
        // Apply envelope
        let envelope = self.noise_envelope.process();
        
        // Configure snare EQ (for the 2kHz bump)
        self.snare_eq.configure(
            self.params.snare_params.eq_freq.smoothed.next(), 
            self.params.snare_params.eq_gain.smoothed.next(),
            self.params.snare_params.eq_q.smoothed.next(),
            self.sample_rate
        );
        
        // Apply EQ and level control
        let output = noise * envelope * self.params.snare_params.level.smoothed.next();
        self.snare_eq.process(output)
    }
    
    fn process_resonance(&mut self, transient_output: f32, snare_output: f32) -> f32 {
        // Get delay samples directly (not time-based)
        let delay_samples = self.params.tuning_params.delay_samples.smoothed.next() as usize;
        
        // Ensure delay is within buffer size
        let delay_samples = delay_samples.min(MAX_DELAY - 1);
        
        // Set read position based on current write position and delay
        self.resonance_read_pos = (self.resonance_write_pos + MAX_DELAY - delay_samples) % MAX_DELAY;
        
        // Read from delay buffer at the delayed position
        let delayed_sample = self.resonance_buffer[self.resonance_read_pos];
        
        // Apply lowpass filter (damping) to feedback - key part of Karplus-Strong
        let damping = self.params.tuning_params.damping.smoothed.next();
        self.resonance_lowpass.set_cutoff(1.0 - damping, true);
        let filtered_feedback = self.resonance_lowpass.process(delayed_sample);
        
        // Apply feedback - note the negative feedback for resonance
        let feedback = self.params.tuning_params.feedback.smoothed.next();
        
        // Mix transient + snare input with filtered feedback
        // Both transient and snare noise feed into the resonator
        let resonance_input = transient_output + snare_output + (filtered_feedback * feedback);
        
        // Write to buffer
        self.resonance_buffer[self.resonance_write_pos] = resonance_input;
        
        // Update write position
        self.resonance_write_pos = (self.resonance_write_pos + 1) % MAX_DELAY;
        
        // Configure resonance EQ
        self.resonance_eq.configure(
            self.params.tuning_params.eq_freq.smoothed.next(), 
            self.params.tuning_params.eq_gain.smoothed.next(),
            self.params.tuning_params.eq_q.smoothed.next(),
            self.sample_rate
        );
        
        // Apply EQ and level control
        let output = resonance_input * self.params.tuning_params.level.smoothed.next();
        self.resonance_eq.process(output)
    }
}

impl Plugin for DrumSynth {
    const NAME: &'static str = "Drum Synth";
    const VENDOR: &'static str = "r-cha";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "info@archasolutions.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // The first audio IO layout is used as the default. The other layouts may be selected either
    // explicitly or automatically by the host or the user depending on the plugin API/backend.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        self.transient_envelope.sample_rate = buffer_config.sample_rate;
        self.noise_envelope.sample_rate = buffer_config.sample_rate;
        
        // Configure EQs with initial values
        self.transient_eq.configure(
            self.params.impact_params.eq_freq.value(),
            self.params.impact_params.eq_gain.value(),
            self.params.impact_params.eq_q.value(),
            buffer_config.sample_rate
        );
        
        self.resonance_eq.configure(
            self.params.tuning_params.eq_freq.value(),
            self.params.tuning_params.eq_gain.value(),
            self.params.tuning_params.eq_q.value(),
            buffer_config.sample_rate
        );
        
        self.snare_eq.configure(
            self.params.snare_params.eq_freq.value(),
            self.params.snare_params.eq_gain.value(),
            self.params.snare_params.eq_q.value(),
            buffer_config.sample_rate
        );

        true
    }

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }
    
    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        ui::default_editor(self.params.clone(), self.editor_state.clone())
    }

    fn reset(&mut self) {
        self.midi_note_id = 0;
        self.midi_note_freq = 1.0;
        self.is_playing = false;
        self.transient_envelope.state = ADSRState::Idle;
        self.noise_envelope.state = ADSRState::Idle;
        
        // Reset filter states
        self.transient_eq.reset();
        self.resonance_eq.reset();
        self.resonance_lowpass.reset();
        self.snare_eq.reset();
        
        // Clear resonance buffer
        for sample in &mut self.resonance_buffer {
            *sample = 0.0;
        }
        self.resonance_write_pos = 0;
        self.resonance_read_pos = 0;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut next_event = context.next_event();
        
        // Update ADSR parameters
        self.transient_envelope.set_parameters(
            self.params.impact_params.attack.value(),
            self.params.impact_params.decay.value(),
            0.0, // -inf sustain for transient
            self.params.impact_params.release.value(),
            self.params.impact_params.hold.value(),
        );
        
        self.noise_envelope.set_parameters(
            self.params.snare_params.attack.value(),
            self.params.snare_params.decay.value(),
            0.0, // No sustain for snare
            self.params.snare_params.decay.value() * 0.5, // Shorter release
            0.0, // No hold
        );
        
        for (sample_id, channel_samples) in buffer.iter_samples().enumerate() {
            // Handle MIDI events
            while let Some(event) = next_event {
                if event.timing() > sample_id as u32 {
                    break;
                }

                match event {
                    NoteEvent::NoteOn { note, .. } => {
                        self.midi_note_id = note;
                        self.midi_note_freq = util::midi_note_to_freq(note);
                        self.is_playing = true;
                        
                        // Trigger envelopes
                        self.transient_envelope.note_on();
                        self.noise_envelope.note_on();
                    }
                    NoteEvent::NoteOff { note, .. } if note == self.midi_note_id => {
                        self.transient_envelope.note_off();
                        self.noise_envelope.note_off();
                    }
                    _ => (),
                }

                next_event = context.next_event();
            }
            
            // Process each layer - snare feeds through resonator per Karplus-Strong
            let transient_output = self.process_transient();
            let snare_output = self.process_snare_input();
            let resonance_output = self.process_resonance(transient_output, snare_output);
            
            // Resonance output contains both transient and snare processed through delay
            let output = (transient_output + resonance_output) 
                * util::db_to_gain_fast(self.params.gain.smoothed.next());
                
            // Apply to all channels
            for sample in channel_samples {
                *sample = output;
            }
            
            // Check if we're still active
            self.is_playing = self.transient_envelope.is_active() || self.noise_envelope.is_active();
        }

        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for DrumSynth {
    const CLAP_ID: &'static str = "com.r-cha.dev.drum-synth";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Realistic synthetic drums");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;

    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Drum,
    ];
}

impl Vst3Plugin for DrumSynth {
    const VST3_CLASS_ID: [u8; 16] = *b"rchadrumsynth000";

    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Synth,
        Vst3SubCategory::Drum,
    ];
}

nih_export_clap!(DrumSynth);
nih_export_vst3!(DrumSynth);