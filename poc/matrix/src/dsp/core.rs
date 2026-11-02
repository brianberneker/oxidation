use super::state::{BorrowedState, OriginalState};
use crate::parameters::{NodeParameters, Parameters};
use std::marker::PhantomData;

pub struct DspCore;

impl DspCore {
    #[inline(always)]
    pub fn matrixverb_sample([a, b, c, d]: [f64; 4], parameters: Parameters) -> [f64; 4] {
        // Share only identical subexpressions, without reassociating any sum.
        let ab = a + b;
        let ac = a + c;
        let bc = b + c;
        [
            (a - (bc + d)) * parameters.regeneration,
            (b - (ac + d)) * parameters.regeneration,
            (c - (ab + d)) * parameters.regeneration,
            (d - (ab + c)) * parameters.regeneration,
        ]
    }
}

#[derive(Default)]
pub struct OriginalNode {
    state: OriginalState,
}

pub trait MatrixMath {
    fn transform(input: [f32; 4]) -> [f32; 4];

    #[inline(always)]
    fn transform_accelerated(input: [f32; 4]) -> [f32; 4] {
        Self::transform(input)
    }
}

#[derive(Default)]
pub struct OriginalOrder;

impl MatrixMath for OriginalOrder {
    #[inline(always)]
    fn transform(input: [f32; 4]) -> [f32; 4] {
        DspCore::ultralight3_sample(input)
    }

    #[inline(always)]
    fn transform_accelerated([a, b, c, d]: [f32; 4]) -> [f32; 4] {
        let negative_sum = -(a + b + c + d);
        [
            2.0_f32.mul_add(a, negative_sum),
            2.0_f32.mul_add(b, negative_sum),
            2.0_f32.mul_add(c, negative_sum),
            2.0_f32.mul_add(d, negative_sum),
        ]
    }
}

#[derive(Default)]
pub struct Balanced;

impl MatrixMath for Balanced {
    #[inline(always)]
    fn transform([a, b, c, d]: [f32; 4]) -> [f32; 4] {
        let negative_sum = -((a + b) + (c + d));
        [
            a * 2.0 + negative_sum,
            b * 2.0 + negative_sum,
            c * 2.0 + negative_sum,
            d * 2.0 + negative_sum,
        ]
    }

    #[inline(always)]
    fn transform_accelerated([a, b, c, d]: [f32; 4]) -> [f32; 4] {
        let negative_sum = -((a + b) + (c + d));
        [
            2.0_f32.mul_add(a, negative_sum),
            2.0_f32.mul_add(b, negative_sum),
            2.0_f32.mul_add(c, negative_sum),
            2.0_f32.mul_add(d, negative_sum),
        ]
    }
}

#[derive(Default)]
pub struct Node<M: MatrixMath> {
    state: BorrowedState,
    math: PhantomData<M>,
}

pub type BorrowedNode = Node<OriginalOrder>;
pub type ApproximateNode = Node<Balanced>;

#[inline(always)]
fn lanes<'a>(
    a: &'a mut [f32],
    b: &'a mut [f32],
    c: &'a mut [f32],
    d: &'a mut [f32],
) -> impl Iterator<Item = (&'a mut f32, &'a mut f32, &'a mut f32, &'a mut f32)> {
    a.iter_mut()
        .zip(b)
        .zip(c)
        .zip(d)
        .map(|(((a, b), c), d)| (a, b, c, d))
}

#[inline(always)]
fn filter_feedback(
    feedback: &mut [f32; 4],
    high: &mut [f32; 4],
    low: &mut [f32; 4],
    parameters: NodeParameters,
    accelerated: bool,
) {
    feedback
        .iter_mut()
        .zip(high)
        .skip(parameters.high_pass_start())
        .for_each(|(feedback, state)| {
            *state *= 0.25;
            *feedback = if accelerated {
                (-*state).mul_add(0.125, *feedback)
            } else {
                *feedback - *state * 0.125
            };
            *state += *feedback;
        });
    feedback
        .iter_mut()
        .zip(low)
        .take(parameters.low_pass_count())
        .for_each(|(feedback, state)| {
            *feedback = (*feedback + *state) * 0.5;
            *state = *feedback;
        });
}

#[inline(always)]
fn node_outputs(floor: [f32; 4], ceiling: [f32; 4]) -> [f32; 8] {
    let [a, b, c, d] = floor.map(|x| x * 0.25);
    let [e, f, g, h] = ceiling.map(|x| x * 0.125);
    [a, b, c, d, e, f, g, h]
}

impl<M: MatrixMath> Node<M> {
    pub fn reset(&mut self) {
        self.state.reset();
    }

    #[inline(never)]
    pub fn process(&mut self, input: &[f32], output: &mut [[f32; 8]], parameters: NodeParameters) {
        self.process_with_acceleration(input, output, parameters, false);
    }

    #[inline(never)]
    pub fn process_with_acceleration(
        &mut self,
        input: &[f32],
        output: &mut [[f32; 8]],
        parameters: NodeParameters,
        accelerated: bool,
    ) {
        if accelerated {
            self.process_inner::<true>(input, output, parameters);
        } else {
            self.process_inner::<false>(input, output, parameters);
        }
    }

    #[inline(never)]
    fn process_inner<const ACCELERATED: bool>(
        &mut self,
        mut input: &[f32],
        mut output: &mut [[f32; 8]],
        parameters: NodeParameters,
    ) {
        assert_eq!(input.len(), output.len());
        let s = &mut self.state;
        while !input.is_empty() {
            // Stop at the first ring wrap, so all sixteen mutable windows are
            // disjoint and contiguous for the complete inner sample loop.
            let length = [
                input.len(),
                s.a.available(),
                s.b.available(),
                s.c.available(),
                s.d.available(),
                s.e.available(),
                s.f.available(),
                s.g.available(),
                s.h.available(),
                s.i.available(),
                s.j.available(),
                s.k.available(),
                s.l.available(),
                s.m.available(),
                s.n.available(),
                s.o.available(),
                s.p.available(),
            ]
            .into_iter()
            .min()
            .unwrap();
            let (source, remaining_input) = input.split_at(length);
            let (destination, remaining_output) = output.split_at_mut(length);
            let first = lanes(
                s.a.window(length),
                s.b.window(length),
                s.c.window(length),
                s.d.window(length),
            );
            let second = lanes(
                s.e.window(length),
                s.f.window(length),
                s.g.window(length),
                s.h.window(length),
            );
            let third = lanes(
                s.i.window(length),
                s.j.window(length),
                s.k.window(length),
                s.l.window(length),
            );
            let fourth = lanes(
                s.m.window(length),
                s.n.window(length),
                s.o.window(length),
                s.p.window(length),
            );
            source
                .iter()
                .zip(destination)
                .zip(first)
                .zip(second)
                .zip(third)
                .zip(fourth)
                .for_each(
                    |(
                        ((((&input, output), (a, b, c, d)), (e, f, g, h)), (i, j, k, l)),
                        (m, n, o, p),
                    )| {
                        let stage1 = [*a, *b, *c, *d];
                        let stage2 = [*e, *f, *g, *h];
                        let floor = [*i, *j, *k, *l];
                        let ceiling = [*m, *n, *o, *p];
                        let [fa, fb, fc, fd] = s.feedback;
                        *a = if ACCELERATED {
                            fa.mul_add(parameters.damping(), input)
                        } else {
                            fa * parameters.damping() + input
                        };
                        *b = if ACCELERATED {
                            fb.mul_add(parameters.damping(), input)
                        } else {
                            fb * parameters.damping() + input
                        };
                        *c = if ACCELERATED {
                            fc.mul_add(parameters.damping(), input)
                        } else {
                            fc * parameters.damping() + input
                        };
                        *d = if ACCELERATED {
                            fd.mul_add(parameters.damping(), input)
                        } else {
                            fd * parameters.damping() + input
                        };
                        [*e, *f, *g, *h] = if ACCELERATED {
                            M::transform_accelerated(stage1)
                        } else {
                            M::transform(stage1)
                        };
                        [*i, *j, *k, *l] = if ACCELERATED {
                            M::transform_accelerated(stage2)
                        } else {
                            M::transform(stage2)
                        };
                        [*m, *n, *o, *p] = if ACCELERATED {
                            M::transform_accelerated(floor)
                        } else {
                            M::transform(floor)
                        };
                        s.feedback = if ACCELERATED {
                            M::transform_accelerated(ceiling)
                        } else {
                            M::transform(ceiling)
                        };
                        filter_feedback(
                            &mut s.feedback,
                            &mut s.high,
                            &mut s.low,
                            parameters,
                            ACCELERATED,
                        );
                        *output = node_outputs(floor, ceiling);
                    },
                );
            input = remaining_input;
            output = remaining_output;
        }
    }
}

impl OriginalNode {
    pub fn reset(&mut self) {
        self.state.reset();
    }

    #[inline(always)]
    fn sample(&mut self, input: f32, parameters: NodeParameters) -> [f32; 8] {
        let s = &mut self.state;
        let [fa, fb, fc, fd] = s.feedback;
        let damping = parameters.damping();
        let first = [
            s.a.original_tick(fa * damping + input),
            s.b.original_tick(fb * damping + input),
            s.c.original_tick(fc * damping + input),
            s.d.original_tick(fd * damping + input),
        ];
        let [a, b, c, d] = DspCore::ultralight3_sample(first);
        let second = [
            s.e.original_tick(a),
            s.f.original_tick(b),
            s.g.original_tick(c),
            s.h.original_tick(d),
        ];
        let [a, b, c, d] = DspCore::ultralight3_sample(second);
        let floor = [
            s.i.original_tick(a),
            s.j.original_tick(b),
            s.k.original_tick(c),
            s.l.original_tick(d),
        ];
        let [a, b, c, d] = DspCore::ultralight3_sample(floor);
        let ceiling = [
            s.m.original_tick(a),
            s.n.original_tick(b),
            s.o.original_tick(c),
            s.p.original_tick(d),
        ];
        s.feedback = DspCore::ultralight3_sample(ceiling);
        filter_feedback(&mut s.feedback, &mut s.high, &mut s.low, parameters, false);
        node_outputs(floor, ceiling)
    }

    #[inline(never)]
    pub fn process(&mut self, input: &[f32], output: &mut [[f32; 8]], parameters: NodeParameters) {
        assert_eq!(input.len(), output.len());
        input.iter().zip(output).for_each(|(&input, output)| {
            *output = self.sample(input, parameters);
        });
    }
}

impl DspCore {
    #[inline(always)]
    pub fn ultralight3_sample([a, b, c, d]: [f32; 4]) -> [f32; 4] {
        // The left-associated reduction is part of the bit-identical contract.
        let negative_sum = -(a + b + c + d);
        [
            a * 2.0 + negative_sum,
            b * 2.0 + negative_sum,
            c * 2.0 + negative_sum,
            d * 2.0 + negative_sum,
        ]
    }

    #[inline(never)]
    pub fn matrixverb(input: &[[f64; 4]], output: &mut [[f64; 4]], parameters: Parameters) {
        assert_eq!(input.len(), output.len());
        input.iter().zip(output).for_each(|(&frame, out)| {
            *out = Self::matrixverb_sample(frame, parameters);
        });
    }

    #[inline(never)]
    pub fn ultralight3(input: &[[f32; 4]], output: &mut [[f32; 4]]) {
        assert_eq!(input.len(), output.len());
        input.iter().zip(output).for_each(|(&frame, out)| {
            *out = Self::ultralight3_sample(frame);
        });
    }
}
