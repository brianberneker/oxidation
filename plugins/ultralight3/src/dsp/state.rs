use oxidation_dsp::dsp::core::{ApproximateNode, BorrowedNode};

pub const HOST_BLOCK: usize = 64;
pub const NATIVE_BLOCK: usize = HOST_BLOCK * 6;

pub struct PreDelay {
    pub q: [f32; 1885],
    pub r: [f32; 915],
    pub s: [f32; 625],
    pub t: [f32; 7925],
    pub cursors: [usize; 4],
    pub outputs: [f32; 4],
}

impl Default for PreDelay {
    fn default() -> Self {
        Self {
            q: [0.0; 1885],
            r: [0.0; 915],
            s: [0.0; 625],
            t: [0.0; 7925],
            cursors: [1; 4],
            outputs: [0.0; 4],
        }
    }
}

pub struct DspState {
    pub exact: BorrowedNode,
    pub approximate: ApproximateNode,
    pub mono: [f32; NATIVE_BLOCK],
    pub dry: [[f32; 2]; NATIVE_BLOCK],
    pub exact_output: [[f32; 8]; NATIVE_BLOCK],
    pub approximate_output: [[f32; 8]; NATIVE_BLOCK],
    pub exact_wet: [[f32; 2]; NATIVE_BLOCK + 2],
    pub approximate_wet: [[f32; 2]; NATIVE_BLOCK + 2],
    pub positions: [(usize, f32); HOST_BLOCK],
    pub phase: f32,
    pub previous_input: [f32; 2],
    pub mode_mix: f32,
    pub predelay: PreDelay,
    pub distance_left: [f32; 8],
    pub distance_right: [f32; 8],
    pub denormal: [u32; 2],
    pub dither: [u32; 2],
}

impl Default for DspState {
    fn default() -> Self {
        Self {
            exact: BorrowedNode::default(),
            approximate: ApproximateNode::default(),
            mono: [0.0; NATIVE_BLOCK],
            dry: [[0.0; 2]; NATIVE_BLOCK],
            exact_output: [[0.0; 8]; NATIVE_BLOCK],
            approximate_output: [[0.0; 8]; NATIVE_BLOCK],
            exact_wet: [[0.0; 2]; NATIVE_BLOCK + 2],
            approximate_wet: [[0.0; 2]; NATIVE_BLOCK + 2],
            positions: [(0, 0.0); HOST_BLOCK],
            phase: 0.0,
            previous_input: [0.0; 2],
            mode_mix: 0.0,
            predelay: PreDelay::default(),
            distance_left: [0.0; 8],
            distance_right: [0.0; 8],
            denormal: [0x1357_9bdf, 0xfdb9_7531],
            dither: [0x1234_5678, 0x8765_4321],
        }
    }
}

impl DspState {
    pub fn reset(&mut self, original_order: bool) {
        *self = Self::default();
        self.mode_mix = f32::from(!original_order);
    }
}
