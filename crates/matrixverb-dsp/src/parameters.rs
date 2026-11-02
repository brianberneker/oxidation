#[derive(Clone, Copy, Debug)]
pub struct ProcessingParameters {
    pub filter: f32,
    pub damping: f32,
    pub speed: f32,
    pub vibrato: f32,
    pub room_size: f32,
    pub flavor: f32,
    pub wet: f32,
}

impl Default for ProcessingParameters {
    fn default() -> Self {
        Self {
            filter: 1.0,
            damping: 0.0,
            speed: 0.0,
            vibrato: 0.0,
            room_size: 0.5,
            flavor: 0.5,
            wet: 1.0,
        }
    }
}

impl ProcessingParameters {
    pub fn validate(self) -> Result<Self, &'static str> {
        let values = [
            self.filter,
            self.damping,
            self.speed,
            self.vibrato,
            self.room_size,
            self.flavor,
            self.wet,
        ];
        if values
            .iter()
            .all(|x| x.is_finite() && (0.0..=1.0).contains(x))
        {
            Ok(self)
        } else {
            Err("MatrixVerb controls must be finite values between zero and one")
        }
    }
}
