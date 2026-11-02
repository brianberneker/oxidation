use nih_plug::prelude::*;
use oxidation_matrixverb_dsp::dsp::core::DspCore;
use parameters::MatrixVerbParams;
use std::sync::Arc;

mod parameters;

const BLOCK: usize = 64;

struct MatrixVerb {
    params: Arc<MatrixVerbParams>,
    core: DspCore,
    input: [[f32; 2]; BLOCK],
    output: [[f32; 2]; BLOCK],
}

impl Default for MatrixVerb {
    fn default() -> Self {
        Self {
            params: Arc::new(MatrixVerbParams::default()),
            core: DspCore::default(),
            input: [[0.0; 2]; BLOCK],
            output: [[0.0; 2]; BLOCK],
        }
    }
}

impl Plugin for MatrixVerb {
    const NAME: &'static str = "oxidation MatrixVerb";
    const VENDOR: &'static str = "Custom DSP";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
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
        _layout: &AudioIOLayout,
        config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        nih_log!(
            "MatrixVerb CPU acceleration backend: {}",
            oxidation_dsp::acceleration::backend()
        );
        match self.core.init(
            f64::from(config.sample_rate),
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
        let channels = buffer.as_slice();
        let (left, right) = channels.split_at_mut(1);
        for (left, right) in left[0].chunks_mut(BLOCK).zip(right[0].chunks_mut(BLOCK)) {
            let count = left.len();
            left.iter()
                .zip(right.iter())
                .zip(&mut self.input)
                .for_each(|((&l, &r), frame)| {
                    *frame = [l, r];
                });
            if let Err(error) = self.core.process(
                &self.input[..count],
                &mut self.output[..count],
                self.params.processing(),
                self.params.original_order.value(),
                self.params.acceleration.value(),
            ) {
                nih_error!("{error}");
                return ProcessStatus::Error(error);
            }
            left.iter_mut()
                .zip(right)
                .zip(&self.output)
                .for_each(|((l, r), frame)| {
                    *l = frame[0];
                    *r = frame[1];
                });
        }
        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for MatrixVerb {
    const CLAP_ID: &'static str = "audio.oxidation.matrixverb";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("MatrixVerb with selectable original math, polynomial math, and CPU SIMD/FMA");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Reverb,
    ];
}

impl Vst3Plugin for MatrixVerb {
    const VST3_CLASS_ID: [u8; 16] = *b"OxidationMatrix1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Reverb];
}

nih_export_clap!(MatrixVerb);
nih_export_vst3!(MatrixVerb);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_and_parameter_ids_are_stable() {
        assert_eq!(MatrixVerb::NAME, "oxidation MatrixVerb");
        assert_eq!(MatrixVerb::CLAP_ID, "audio.oxidation.matrixverb");
        assert_eq!(MatrixVerb::VST3_CLASS_ID, *b"OxidationMatrix1");
        let params = MatrixVerbParams::default();
        let ids: Vec<_> = params
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        assert_eq!(
            ids,
            [
                "filter",
                "damping",
                "speed",
                "vibrato",
                "room_size",
                "flavor",
                "wet",
                "original_order",
                "acceleration"
            ]
        );
        assert_eq!(params.processing().filter, 1.0);
        assert_eq!(params.processing().wet, 1.0);
        assert!(params.original_order.value());
        assert!(!params.acceleration.value());
    }
}
