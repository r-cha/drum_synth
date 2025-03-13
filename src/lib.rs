use nih_plug::prelude::*;
use rand::Rng;
use std::sync::Arc;

/// The maximum size of a delay buffer for resonance.
/// ~100ms at 44.1kHz sample rate
const MAX_DELAY: usize = 4096;

struct DrumSynth {
    params: Arc<DrumSynthParams>,
    sample_rate: f32,

    // Transient layer
    transient_envelope: ADSREnvelope,
    transient_phase: f32,

    // Resonance layer
    resonance_buffer: Vec<f32>,
    resonance_write_pos: usize,
    resonance_read_pos: usize,

    // Noise for snares
    noise_envelope: ADSREnvelope,
    
    // MIDI tracking
    midi_note_id: u8,
    midi_note_freq: f32,
    is_playing: bool,
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
        match self.state {
            ADSRState::Idle => false,
            _ => true,
        }
    }
}

#[derive(Params)]
struct DrumSynthParams {
    #[id = "gain"]
    pub gain: FloatParam,

    // Transient layer params
    #[id = "tr_attack"]
    pub transient_attack: FloatParam,
    
    #[id = "tr_hold"]
    pub transient_hold: FloatParam,
    
    #[id = "tr_decay"]
    pub transient_decay: FloatParam,
    
    #[id = "tr_release"]
    pub transient_release: FloatParam,
    
    #[id = "tr_level"]
    pub transient_level: FloatParam,

    // Resonance layer params
    #[id = "res_delay"]
    pub resonance_delay: FloatParam,
    
    #[id = "res_feedback"]
    pub resonance_feedback: FloatParam,
    
    #[id = "res_level"]
    pub resonance_level: FloatParam,

    // Snare layer params
    #[id = "snare_attack"]
    pub snare_attack: FloatParam,
    
    #[id = "snare_decay"]
    pub snare_decay: FloatParam,
    
    #[id = "snare_level"] 
    pub snare_level: FloatParam,
    
    // Overall tone controls
    #[id = "pitch"]
    pub pitch: FloatParam,
}

impl Default for DrumSynth {
    fn default() -> Self {
        Self {
            params: Arc::new(DrumSynthParams::default()),
            sample_rate: 44100.0,

            transient_envelope: ADSREnvelope::new(44100.0),
            transient_phase: 0.0,

            resonance_buffer: vec![0.0; MAX_DELAY],
            resonance_write_pos: 0,
            resonance_read_pos: 0,

            noise_envelope: ADSREnvelope::new(44100.0),
            
            midi_note_id: 0,
            midi_note_freq: 1.0,
            is_playing: false,
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
            
            // Transient layer params (square osc)
            transient_attack: FloatParam::new(
                "Transient Attack",
                0.0005, // 0.5ms
                FloatRange::Skewed {
                    min: 0.0001,
                    max: 0.01,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" s"),
            
            transient_hold: FloatParam::new(
                "Transient Hold",
                0.0, // 0ms
                FloatRange::Skewed {
                    min: 0.0,
                    max: 0.01,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" s"),
            
            transient_decay: FloatParam::new(
                "Transient Decay",
                0.02, // 20ms
                FloatRange::Skewed {
                    min: 0.01,
                    max: 0.03,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" s"),
            
            transient_release: FloatParam::new(
                "Transient Release",
                0.015, // 15ms
                FloatRange::Skewed {
                    min: 0.01,
                    max: 0.03,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" s"),
            
            transient_level: FloatParam::new(
                "Transient Level",
                0.8,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1.0,
                },
            ),
            
            // Resonance layer params
            resonance_delay: FloatParam::new(
                "Resonance Delay",
                0.001, // 1ms
                FloatRange::Skewed {
                    min: 0.0001,
                    max: 0.01,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" s"),
            
            resonance_feedback: FloatParam::new(
                "Resonance Feedback",
                -0.7,
                FloatRange::Linear {
                    min: -0.99,
                    max: -0.3,
                },
            ),
            
            resonance_level: FloatParam::new(
                "Resonance Level",
                0.8,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1.0,
                },
            ),
            
            // Snare layer params
            snare_attack: FloatParam::new(
                "Snare Attack",
                0.001,
                FloatRange::Skewed {
                    min: 0.0001,
                    max: 0.01,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" s"),
            
            snare_decay: FloatParam::new(
                "Snare Decay",
                0.1,
                FloatRange::Skewed {
                    min: 0.01,
                    max: 0.5,
                    factor: FloatRange::skew_factor(-1.0)
                },
            )
            .with_unit(" s"),
            
            snare_level: FloatParam::new(
                "Snare Level",
                0.3,
                FloatRange::Linear {
                    min: 0.0,
                    max: 1.0,
                },
            ),
            
            // Overall tone controls
            pitch: FloatParam::new(
                "Pitch",
                60.0, // Middle C
                FloatRange::Linear {
                    min: 36.0, // C2
                    max: 84.0, // C6
                },
            )
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_step_size(1.0)
            .with_unit(""),
        }
    }
}

impl DrumSynth {
    fn calculate_square(&mut self, frequency: f32) -> f32 {
        let phase_delta = frequency / self.sample_rate;
        
        // Simple square wave
        let square = if self.transient_phase < 0.5 { 1.0 } else { -1.0 };

        self.transient_phase += phase_delta;
        if self.transient_phase >= 1.0 {
            self.transient_phase -= 1.0;
        }

        square
    }
    
    fn calculate_noise() -> f32 {
        let mut rng = rand::thread_rng();
        rng.gen_range(-1.0..1.0)
    }
    
    fn process_transient(&mut self, frequency: f32) -> f32 {
        // Get square wave for transient
        let square = self.calculate_square(frequency);
        
        // Apply envelope to transient
        let envelope = self.transient_envelope.process();
        
        square * envelope * self.params.transient_level.smoothed.next()
    }
    
    fn process_resonance(&mut self, transient_output: f32) -> f32 {
        // Calculate delay samples based on pitch
        let delay_samples = (self.params.resonance_delay.smoothed.next() * self.sample_rate) as usize;
        
        // Ensure delay is within buffer size
        let delay_samples = delay_samples.min(MAX_DELAY - 1);
        
        // Set read position based on current write position and delay
        self.resonance_read_pos = (self.resonance_write_pos + MAX_DELAY - delay_samples) % MAX_DELAY;
        
        // Read from delay buffer at the delayed position
        let delayed_sample = self.resonance_buffer[self.resonance_read_pos];
        
        // Apply feedback - note the negative feedback for resonance
        let feedback = self.params.resonance_feedback.smoothed.next();
        
        // Mix transient input with feedback
        let resonance_input = transient_output + (delayed_sample * feedback);
        
        // Write to buffer
        self.resonance_buffer[self.resonance_write_pos] = resonance_input;
        
        // Update write position
        self.resonance_write_pos = (self.resonance_write_pos + 1) % MAX_DELAY;
        
        // Output with level control
        resonance_input * self.params.resonance_level.smoothed.next()
    }
    
    fn process_snare(&mut self) -> f32 {
        // Generate noise for snare
        let noise = Self::calculate_noise();
        
        // Apply envelope
        let envelope = self.noise_envelope.process();
        
        // Apply level control
        noise * envelope * self.params.snare_level.smoothed.next()
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

        true
    }

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn reset(&mut self) {
        self.transient_phase = 0.0;
        self.midi_note_id = 0;
        self.midi_note_freq = 1.0;
        self.is_playing = false;
        self.transient_envelope.state = ADSRState::Idle;
        self.noise_envelope.state = ADSRState::Idle;
        
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
            self.params.transient_attack.value(),
            self.params.transient_decay.value(),
            0.0, // -inf sustain for transient
            self.params.transient_release.value(),
            self.params.transient_hold.value(),
        );
        
        self.noise_envelope.set_parameters(
            self.params.snare_attack.value(),
            self.params.snare_decay.value(),
            0.0, // No sustain for snare
            self.params.snare_decay.value() * 0.5, // Shorter release
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
            
            // Determine frequency (either from MIDI or from pitch parameter)
            let frequency = if self.is_playing {
                self.midi_note_freq
            } else {
                util::midi_note_to_freq(self.params.pitch.smoothed.next() as u8)
            };
            
            // Process each layer
            let transient_output = self.process_transient(frequency);
            let resonance_output = self.process_resonance(transient_output);
            let snare_output = self.process_snare();
            
            // Mix all layers
            let output = (transient_output + resonance_output + snare_output) 
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