use nih_plug::prelude::*;

#[derive(Params)]
pub struct OxidationParams {
    #[id = "damping"]
    pub damping: IntParam,
    #[id = "unsolid"]
    pub unsolid: IntParam,
    #[id = "unreflect"]
    pub unreflect: IntParam,
    #[id = "predelay"]
    pub predelay: IntParam,
    #[id = "wet"]
    pub wet: FloatParam,
    #[id = "front_back"]
    pub front_back: FloatParam,
    #[id = "bottom_top"]
    pub bottom_top: FloatParam,
    #[id = "distance"]
    pub distance: FloatParam,
    #[id = "original_order"]
    pub original_order: BoolParam,
    #[id = "acceleration"]
    pub acceleration: BoolParam,
}

impl Default for OxidationParams {
    fn default() -> Self {
        Self {
            damping: IntParam::new("Damping", 2, IntRange::Linear { min: 0, max: 9 }),
            unsolid: IntParam::new("UnSolid", 4, IntRange::Linear { min: 0, max: 4 }),
            unreflect: IntParam::new("UnReflc", 4, IntRange::Linear { min: 0, max: 4 }),
            predelay: IntParam::new("Predlay", 0, IntRange::Linear { min: 0, max: 998 }),
            wet: FloatParam::new("Dry/Wet", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            front_back: FloatParam::new("Frnt/Bk", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            bottom_top: FloatParam::new("Bot/Top", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            distance: FloatParam::new("Distnce", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            original_order: BoolParam::new("Original-order math", true),
            acceleration: BoolParam::new("Acceleration", false),
        }
    }
}
