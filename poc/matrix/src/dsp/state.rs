pub struct Delay<const N: usize> {
    samples: [f32; N],
    cursor: usize,
}

impl<const N: usize> Default for Delay<N> {
    fn default() -> Self {
        Self {
            samples: [0.0; N],
            cursor: 1,
        }
    }
}

impl<const N: usize> Delay<N> {
    #[inline(always)]
    pub fn original_tick(&mut self, input: f32) -> f32 {
        self.samples[self.cursor] = input;
        let next = self.cursor + 1;
        self.cursor = next * usize::from(next < N);
        self.samples[self.cursor]
    }
}

pub struct Ring<const N: usize> {
    samples: [f32; N],
    cursor: usize,
}

impl<const N: usize> Default for Ring<N> {
    fn default() -> Self {
        Self {
            samples: [0.0; N],
            cursor: 0,
        }
    }
}

impl<const N: usize> Ring<N> {
    #[inline(always)]
    pub fn available(&self) -> usize {
        N - self.cursor
    }

    #[inline(always)]
    pub fn window(&mut self, length: usize) -> &mut [f32] {
        let start = self.cursor;
        let end = start + length;
        assert!(end <= N);
        self.cursor = end * usize::from(end < N);
        &mut self.samples[start..end]
    }
}

#[derive(Default)]
pub struct OriginalState {
    pub a: Delay<1627>,
    pub b: Delay<651>,
    pub c: Delay<471>,
    pub d: Delay<48>,
    pub e: Delay<1150>,
    pub f: Delay<576>,
    pub g: Delay<457>,
    pub h: Delay<53>,
    pub i: Delay<1757>,
    pub j: Delay<889>,
    pub k: Delay<174>,
    pub l: Delay<50>,
    pub m: Delay<1799>,
    pub n: Delay<777>,
    pub o: Delay<153>,
    pub p: Delay<22>,
    pub feedback: [f32; 4],
    pub high: [f32; 4],
    pub low: [f32; 4],
}

#[derive(Default)]
pub struct BorrowedState {
    pub a: Ring<1626>,
    pub b: Ring<650>,
    pub c: Ring<470>,
    pub d: Ring<47>,
    pub e: Ring<1149>,
    pub f: Ring<575>,
    pub g: Ring<456>,
    pub h: Ring<52>,
    pub i: Ring<1756>,
    pub j: Ring<888>,
    pub k: Ring<173>,
    pub l: Ring<49>,
    pub m: Ring<1798>,
    pub n: Ring<776>,
    pub o: Ring<152>,
    pub p: Ring<21>,
    pub feedback: [f32; 4],
    pub high: [f32; 4],
    pub low: [f32; 4],
}

impl OriginalState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

impl BorrowedState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::{Delay, Ring};

    #[test]
    fn compact_ring_matches_write_advance_read_delay() {
        let mut original = Delay::<4>::default();
        let mut compact = Ring::<3>::default();
        let input: Vec<_> = (0..1000).map(|x| x as f32 * -0.07).collect();
        let expected: Vec<_> = input.iter().map(|&x| original.original_tick(x)).collect();
        let mut actual = vec![0.0; input.len()];
        input.iter().zip(&mut actual).for_each(|(&input, output)| {
            let storage = &mut compact.window(1)[0];
            *output = *storage;
            *storage = input;
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn windows_advance_and_wrap_without_changing_samples() {
        let mut ring = Ring::<7>::default();
        ring.window(7)
            .copy_from_slice(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(ring.available(), 7);
        assert_eq!(ring.window(3), [0.0, 1.0, 2.0]);
        assert_eq!(ring.available(), 4);
        assert_eq!(ring.window(4), [3.0, 4.0, 5.0, 6.0]);
        assert_eq!(ring.available(), 7);
    }
}
