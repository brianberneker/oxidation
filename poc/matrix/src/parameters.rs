#[derive(Clone, Copy)]
pub struct Parameters {
    pub regeneration: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct NodeParameters {
    damping: f32,
    high_pass_start: usize,
    low_pass_count: usize,
}

impl NodeParameters {
    pub fn new(damping_step: u8, unsolid: u8, unreflect: u8) -> Result<Self, &'static str> {
        if damping_step > 9 || unsolid > 4 || unreflect > 4 {
            return Err("node parameters require damping step 0..=9 and tone selectors 0..=4");
        }
        Ok(Self {
            damping: 0.0625 - f32::from(damping_step) * 0.003125,
            high_pass_start: usize::from(unsolid),
            low_pass_count: 4 - usize::from(unreflect),
        })
    }

    #[inline(always)]
    pub fn damping(self) -> f32 {
        self.damping
    }

    pub fn high_pass_start(self) -> usize {
        self.high_pass_start
    }

    pub fn low_pass_count(self) -> usize {
        self.low_pass_count
    }
}
