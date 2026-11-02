use crate::dsp::core::{DspCore, ProcessingParameters};
use crate::dsp::state::HOST_BLOCK;
use crate::parameters::OxidationParams;
use nih_plug::prelude::*;
use oxidation_dsp::parameters::NodeParameters;
use std::sync::Arc;

mod dsp;
mod parameters;
mod utils;

struct Oxidation {
    params: Arc<OxidationParams>,
    core: DspCore,
}

impl Default for Oxidation {
    fn default() -> Self {
        Self {
            params: Arc::new(OxidationParams::default()),
            core: DspCore::new(),
        }
    }
}

impl Plugin for Oxidation {
    const NAME: &'static str = "oxidation";
    const VENDOR: &'static str = "Custom DSP";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = "0.1.0";
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];
    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        nih_log!(
            "oxidation CPU acceleration backend: {}",
            oxidation_dsp::acceleration::backend()
        );
        match self.core.init(
            buffer_config.sample_rate,
            self.params.original_order.value(),
        ) {
            Ok(()) => true,
            Err(error) => {
                nih_error!("{error}");
                false
            }
        }
    }

    fn reset(&mut self) {
        self.core.reset(self.params.original_order.value());
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let parameters = ProcessingParameters {
            node: NodeParameters::new(
                self.params.damping.value() as u8,
                self.params.unsolid.value() as u8,
                self.params.unreflect.value() as u8,
            )
            .expect("host parameter ranges guarantee valid reverb settings"),
            damping_bypass: self.params.damping.value() == 0,
            predelay: self.params.predelay.value() as usize,
            wet: self.params.wet.value(),
            front_back: self.params.front_back.value(),
            bottom_top: self.params.bottom_top.value(),
            distance: self.params.distance.value(),
            original_order: self.params.original_order.value(),
            accelerated: self.params.acceleration.value(),
        };
        let channels = buffer.as_slice();
        let (left_ch, right_ch) = channels.split_at_mut(1);
        left_ch[0]
            .chunks_mut(HOST_BLOCK)
            .zip(right_ch[0].chunks_mut(HOST_BLOCK))
            .for_each(|(left, right)| {
                self.core.process_block(left, right, parameters);
            });

        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for Oxidation {
    const CLAP_ID: &'static str = "audio.oxidation.reverb";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Ultralight3 reverb with selectable Rust math and CPU SIMD/FMA modes");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Reverb,
    ];
}

impl Vst3Plugin for Oxidation {
    const VST3_CLASS_ID: [u8; 16] = *b"Oxidation0000001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Reverb];
}

nih_export_clap!(Oxidation);
nih_export_vst3!(Oxidation);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oxidation_identity_is_stable() {
        assert_eq!(Oxidation::NAME, "oxidation");
        assert_eq!(Oxidation::CLAP_ID, "audio.oxidation.reverb");
        assert_eq!(Oxidation::VST3_CLASS_ID, *b"Oxidation0000001");
    }

    #[test]
    fn upstream_controls_and_rust_toggles_have_stable_parameter_ids() {
        let parameters = OxidationParams::default();
        let ids: Vec<_> = parameters
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        assert_eq!(
            ids,
            [
                "damping",
                "unsolid",
                "unreflect",
                "predelay",
                "wet",
                "front_back",
                "bottom_top",
                "distance",
                "original_order",
                "acceleration"
            ]
        );
        assert_eq!(parameters.damping.value(), 2);
        assert_eq!(parameters.original_order.value(), true);
        assert_eq!(parameters.acceleration.value(), false);
    }
}
