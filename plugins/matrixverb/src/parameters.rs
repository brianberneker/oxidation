use nih_plug::prelude::*;
use oxidation_matrixverb_dsp::parameters::ProcessingParameters;

#[derive(Params)]
pub struct MatrixVerbParams {
    #[id = "filter"]
    pub filter: FloatParam,
    #[id = "damping"]
    pub damping: FloatParam,
    #[id = "speed"]
    pub speed: FloatParam,
    #[id = "vibrato"]
    pub vibrato: FloatParam,
    #[id = "room_size"]
    pub room_size: FloatParam,
    #[id = "flavor"]
    pub flavor: FloatParam,
    #[id = "wet"]
    pub wet: FloatParam,
    #[id = "original_order"]
    pub original_order: BoolParam,
    #[id = "acceleration"]
    pub acceleration: BoolParam,
}

impl Default for MatrixVerbParams {
    fn default() -> Self {
        let range = FloatRange::Linear { min: 0.0, max: 1.0 };
        Self {
            filter: FloatParam::new("Filter", 1.0, range),
            damping: FloatParam::new("Damping", 0.0, range),
            speed: FloatParam::new("Speed", 0.0, range),
            vibrato: FloatParam::new("Vibrato", 0.0, range),
            room_size: FloatParam::new("RmSize", 0.5, range),
            flavor: FloatParam::new("Flavor", 0.5, range),
            wet: FloatParam::new("Dry/Wet", 1.0, range),
            original_order: BoolParam::new("Original-order math", true),
            acceleration: BoolParam::new("Acceleration", false),
        }
    }
}

impl MatrixVerbParams {
    pub fn processing(&self) -> ProcessingParameters {
        ProcessingParameters {
            filter: self.filter.value(),
            damping: self.damping.value(),
            speed: self.speed.value(),
            vibrato: self.vibrato.value(),
            room_size: self.room_size.value(),
            flavor: self.flavor.value(),
            wet: self.wet.value(),
        }
    }
}
