pub const CAPACITIES: [usize; 13] = [
    8111, 7511, 7311, 6911, 6311, 6111, 5511, 4911, 4511, 4311, 3911, 3311, 3111,
];
pub const RATES: [f64; 8] = [
    0.003251, 0.002999, 0.002917, 0.002749, 0.002503, 0.002423, 0.002146, 0.002088,
];
pub const SEEDS: [u32; 2] = [0x1234_5678, 0x8765_4321];
pub const PHASES: [[f64; 2]; 8] = [
    [0.17, 1.91],
    [0.83, 2.71],
    [1.47, 3.37],
    [2.13, 4.03],
    [2.79, 4.69],
    [3.43, 5.33],
    [4.09, 5.99],
    [4.73, 0.37],
];

pub struct DspState {
    pub delays: [Vec<[f64; 2]>; 13],
    pub cursors: [usize; 13],
    pub feedback: [[f64; 2]; 8],
    pub phases: [[f64; 2]; 8],
    pub filters: [[[f64; 2]; 2]; 3],
    pub random: [u32; 2],
}

impl Default for DspState {
    fn default() -> Self {
        Self {
            delays: CAPACITIES.map(|capacity| vec![[0.0; 2]; capacity]),
            cursors: [1; 13],
            feedback: [[0.0; 2]; 8],
            phases: PHASES,
            filters: [[[0.0; 2]; 2]; 3],
            random: SEEDS,
        }
    }
}

impl DspState {
    pub fn reset(&mut self) {
        self.delays
            .iter_mut()
            .for_each(|delay| delay.fill([0.0; 2]));
        self.cursors.fill(1);
        self.feedback.fill([0.0; 2]);
        self.phases = PHASES;
        self.filters.fill([[0.0; 2]; 2]);
        self.random = SEEDS;
    }
}
