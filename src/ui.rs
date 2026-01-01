use crate::DrumSynthParams;
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::{ParamSlider, ParamSliderExt, ParamSliderStyle};
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

#[derive(Lens)]
struct Data {
    params: Arc<DrumSynthParams>,
}

impl Model for Data {}

pub(crate) fn default_editor(params: Arc<DrumSynthParams>, editor_state: Arc<ViziaState>) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        Data {
            params: params.clone(),
        }
        .build(cx);

        // Styling constants
        let bg_color = Color::rgb(20, 20, 20);
        let panel_color = Color::rgb(30, 30, 30);
        let accent_impact = Color::rgb(233, 79, 55);
        let accent_tuning = Color::rgb(30, 136, 229);
        let accent_snare = Color::rgb(67, 160, 71);
        let label_color = Color::rgb(200, 200, 200);

        // Root container
        VStack::new(cx, |cx| {
            // Header
            Label::new(cx, "DRUM SYNTH")
                .font_size(32.0)
                .text_align(TextAlign::Center)
                .color(Color::white())
                .height(Percentage(8.0)) // Relative height
                .width(Stretch(1.0));

            // Main Content Area
            HStack::new(cx, |cx| {
                
                // --- MASTER SECTION (Left Column) ---
                VStack::new(cx, |cx| {
                    Label::new(cx, "MASTER")
                        .font_size(24.0)
                        .color(Color::white())
                        .text_align(TextAlign::Center)
                        .height(Percentage(10.0))
                        .width(Stretch(1.0));

                    // Gain
                    VStack::new(cx, |cx| {
                        Label::new(cx, "Gain").font_size(12.0).color(label_color).text_align(TextAlign::Center);
                        ParamSlider::new(cx, Data::params, |params| &params.gain)
                            .set_style(ParamSliderStyle::CurrentStep { even: true })
                            .width(Stretch(1.0));
                    })
                    .width(Percentage(80.0))
                    .col_between(Percentage(5.0))
                    .height(Percentage(20.0));
                })
                .width(Percentage(20.0)) // 20% width
                .background_color(panel_color)
                .border_radius(Percentage(2.0))
                .child_space(Percentage(2.0))
                .row_between(Percentage(5.0));

                // --- LAYERS (Right Column) ---
                VStack::new(cx, |cx| {
                    
                    // IMPACT LAYER
                    HStack::new(cx, |cx| {
                        // Accent strip
                        Element::new(cx).width(Percentage(1.0)).background_color(accent_impact);

                        // Label
                        Label::new(cx, "IMPACT").font_size(20.0).color(accent_impact).width(Percentage(12.0));

                        // Spacer
                        Element::new(cx).width(Stretch(1.0));

                        // Controls
                        HStack::new(cx, |cx| {
                            make_param(cx, "Atk", |p: &DrumSynthParams| &p.impact_params.attack);
                            make_param(cx, "Hld", |p: &DrumSynthParams| &p.impact_params.hold);
                            make_param(cx, "Dec", |p: &DrumSynthParams| &p.impact_params.decay);
                            make_param(cx, "Rel", |p: &DrumSynthParams| &p.impact_params.release);
                            make_param(cx, "Lvl", |p: &DrumSynthParams| &p.impact_params.level);
                        }).col_between(Percentage(2.0)).width(Percentage(45.0));

                        // Spacer
                        Element::new(cx).width(Stretch(1.0));

                        // EQ
                        HStack::new(cx, |cx| {
                            Label::new(cx, "EQ").font_size(12.0).color(Color::gray()).width(Percentage(15.0));
                            make_param(cx, "F", |p: &DrumSynthParams| &p.impact_params.eq_freq);
                            make_param(cx, "G", |p: &DrumSynthParams| &p.impact_params.eq_gain);
                            make_param(cx, "Q", |p: &DrumSynthParams| &p.impact_params.eq_q);
                        })
                        .background_color(Color::rgb(42, 42, 42))
                        .border_radius(Percentage(5.0))
                        .child_space(Percentage(2.0))
                        .col_between(Percentage(2.0))
                        .width(Percentage(20.0));

                    })
                    .height(Stretch(1.0)) // Distribute height equally
                    .background_color(Color::rgb(37, 37, 37))
                    .border_radius(Percentage(1.0))
                    .col_between(Percentage(2.0))
                    .child_space(Percentage(2.0));

                    // TUNING LAYER
                    HStack::new(cx, |cx| {
                        // Accent strip
                        Element::new(cx).width(Percentage(1.0)).background_color(accent_tuning);

                        // Label
                        Label::new(cx, "TUNING").font_size(20.0).color(accent_tuning).width(Percentage(12.0));

                        // Spacer
                        Element::new(cx).width(Stretch(1.0));

                        // Controls
                        HStack::new(cx, |cx| {
                            make_param(cx, "Ten", |p: &DrumSynthParams| &p.tuning_params.delay_samples);
                            make_param(cx, "Sus", |p: &DrumSynthParams| &p.tuning_params.feedback);
                            make_param(cx, "Dmp", |p: &DrumSynthParams| &p.tuning_params.damping);
                            make_param(cx, "Lvl", |p: &DrumSynthParams| &p.tuning_params.level);
                        }).col_between(Percentage(2.0)).width(Percentage(36.0));
                        
                        // Spacer
                        Element::new(cx).width(Stretch(1.0));

                        // EQ
                        HStack::new(cx, |cx| {
                            Label::new(cx, "EQ").font_size(12.0).color(Color::gray()).width(Percentage(15.0));
                            make_param(cx, "F", |p: &DrumSynthParams| &p.tuning_params.eq_freq);
                            make_param(cx, "G", |p: &DrumSynthParams| &p.tuning_params.eq_gain);
                            make_param(cx, "Q", |p: &DrumSynthParams| &p.tuning_params.eq_q);
                        })
                        .background_color(Color::rgb(42, 42, 42))
                        .border_radius(Percentage(5.0))
                        .child_space(Percentage(2.0))
                        .col_between(Percentage(2.0))
                        .width(Percentage(20.0));

                    })
                    .height(Stretch(1.0))
                    .background_color(Color::rgb(37, 37, 37))
                    .border_radius(Percentage(1.0))
                    .col_between(Percentage(2.0))
                    .child_space(Percentage(2.0));

                    // SNARE LAYER
                    HStack::new(cx, |cx| {
                        // Accent strip
                        Element::new(cx).width(Percentage(1.0)).background_color(accent_snare);

                        // Label
                        Label::new(cx, "SNARE").font_size(20.0).color(accent_snare).width(Percentage(12.0));

                        // Spacer
                        Element::new(cx).width(Stretch(1.0));

                        // Controls
                        HStack::new(cx, |cx| {
                            make_param(cx, "Atk", |p: &DrumSynthParams| &p.snare_params.attack);
                            make_param(cx, "Dec", |p: &DrumSynthParams| &p.snare_params.decay);
                            make_param(cx, "Lvl", |p: &DrumSynthParams| &p.snare_params.level);
                        }).col_between(Percentage(2.0)).width(Percentage(27.0));

                        // Spacer
                        Element::new(cx).width(Stretch(2.0));

                        // EQ
                        HStack::new(cx, |cx| {
                            Label::new(cx, "EQ").font_size(12.0).color(Color::gray()).width(Percentage(15.0));
                            make_param(cx, "F", |p: &DrumSynthParams| &p.snare_params.eq_freq);
                            make_param(cx, "G", |p: &DrumSynthParams| &p.snare_params.eq_gain);
                            make_param(cx, "Q", |p: &DrumSynthParams| &p.snare_params.eq_q);
                        })
                        .background_color(Color::rgb(42, 42, 42))
                        .border_radius(Percentage(5.0))
                        .child_space(Percentage(2.0))
                        .col_between(Percentage(2.0))
                        .width(Percentage(20.0));

                    })
                    .height(Stretch(1.0))
                    .background_color(Color::rgb(37, 37, 37))
                    .border_radius(Percentage(1.0))
                    .col_between(Percentage(2.0))
                    .child_space(Percentage(2.0));

                })
                .width(Percentage(75.0)) // 75% width for layers
                .row_between(Percentage(2.0));
            })
            .height(Stretch(1.0))
            .child_space(Percentage(3.0))
            .col_between(Percentage(3.0));
        })
        .background_color(bg_color);
    })
}

// Helper to create a parameter control block
fn make_param<F>(cx: &mut Context, label: &str, map_fn: F)
where
    F: Fn(&DrumSynthParams) -> &FloatParam + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        Label::new(cx, label).font_size(12.0).color(Color::rgb(200, 200, 200)).text_align(TextAlign::Center);
        ParamSlider::new(cx, Data::params, move |params| map_fn(params))
            .set_style(ParamSliderStyle::CurrentStep { even: true })
            .width(Stretch(1.0)); // Ensure slider stretches to fill container
    })
    .width(Stretch(1.0)) // Stretch to fill available space in the control block
    .col_between(Percentage(5.0));
}
