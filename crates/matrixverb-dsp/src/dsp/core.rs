use super::state::{DspState, RATES};
use crate::{
    parameters::ProcessingParameters,
    utils::math::{arcsine, arcsine_simd, dither, multiply_add, select, sine, sine_simd, xorshift},
};
use oxidation_dsp::acceleration::{f64x2, fma64, splat64, CmpLt};
use oxidation_dsp::{dsp::core::DspCore as MatrixKernel, parameters::Parameters};

const LENGTHS: [f64; 13] = [
    79.0, 73.0, 71.0, 67.0, 61.0, 59.0, 53.0, 47.0, 43.0, 41.0, 37.0, 31.0, 29.0,
];

#[derive(Clone, Copy)]
struct Biquad {
    a0: f64,
    a1: f64,
    b1: f64,
    b2: f64,
}

impl Biquad {
    fn new(k: f64, q: f64) -> Self {
        let norm = 1.0 / (1.0 + k / q + k * k);
        let a0 = k * k * norm;
        Self {
            a0,
            a1: 2.0 * a0,
            b1: 2.0 * (k * k - 1.0) * norm,
            b2: (1.0 - k / q + k * k) * norm,
        }
    }

    #[inline(always)]
    fn sample<const ACCELERATED: bool>(self, input: f64, state: &mut [f64; 2]) -> f64 {
        let output = multiply_add::<ACCELERATED>(input, self.a0, state[0]);
        state[0] = if ACCELERATED {
            multiply_add::<ACCELERATED>(
                -output,
                self.b1,
                multiply_add::<ACCELERATED>(input, self.a1, state[1]),
            )
        } else {
            (input * self.a1) - (output * self.b1) + state[1]
        };
        state[1] = multiply_add::<ACCELERATED>(-output, self.b2, input * self.a0);
        output
    }

    #[inline(always)]
    fn sample_simd(self, input: f64x2, state: &mut [[f64; 2]; 2]) -> f64x2 {
        let z1 = f64x2::new([state[0][0], state[1][0]]);
        let z2 = f64x2::new([state[0][1], state[1][1]]);
        let output = fma64(input, splat64(self.a0), z1);
        let next1 = fma64(
            -output,
            splat64(self.b1),
            fma64(input, splat64(self.a1), z2),
        )
        .to_array();
        let next2 = fma64(-output, splat64(self.b2), input * splat64(self.a0)).to_array();
        *state = [[next1[0], next2[0]], [next1[1], next2[1]]];
        output
    }
}

pub struct Tuning {
    delays: [usize; 13],
    filters: [Biquad; 3],
    speed: f64,
    depth: f64,
    blend: f64,
    crossmod: f64,
    regeneration: f64,
    wet: f64,
}

impl Tuning {
    pub fn new(parameters: ProcessingParameters, sample_rate: f64) -> Result<Self, &'static str> {
        parameters.validate()?;
        if !sample_rate.is_finite() || !(22050.0..=384000.0).contains(&sample_rate) {
            return Err("MatrixVerb requires a sample rate between 22.05 kHz and 384 kHz");
        }
        let size = (f64::from(parameters.room_size).powf(2.0) * 90.0) + 10.0;
        let crossmod = ((f64::from(parameters.flavor) - 0.5) * 2.0).powf(3.0) * 0.5;
        let depth = 1.0
            - (1.0 - (0.82 - ((f64::from(parameters.damping) * 0.5) + (size * 0.002)))).powf(4.0);
        let k = (std::f64::consts::PI
            * (((f64::from(parameters.filter) * 9000.0) + 1000.0) / sample_rate))
            .tan();
        let mut delays = LENGTHS.map(|length| (length * size) as usize);
        delays[12] = ((29.0 * size) - (56.0 * size * crossmod.abs())) as usize;
        Ok(Self {
            delays,
            filters: [
                Biquad::new(k, 1.618_033_988_749_895),
                Biquad::new(k, 0.618_033_988_749_894_9),
                Biquad::new(k, 0.5),
            ],
            speed: 0.06 + f64::from(parameters.speed),
            depth: (0.027 + f64::from(parameters.vibrato).powf(3.0)) * 100.0,
            blend: 0.955 - (size * 0.007),
            crossmod,
            regeneration: depth * (0.5 - (crossmod.abs() * 0.031)),
            wet: f64::from(parameters.wet),
        })
    }
}

#[derive(Default)]
pub struct Processor {
    state: DspState,
}

#[inline(always)]
fn next(cursor: usize, delay: usize) -> usize {
    let next = cursor + 1;
    next * usize::from(next <= delay)
}

#[inline(always)]
fn wrap(cursor: usize, delay: usize) -> usize {
    cursor - usize::from(cursor > delay) * (delay + 1)
}

#[inline(always)]
fn matrix<const APPROXIMATE: bool>(values: [f64; 4], regeneration: f64) -> [f64; 4] {
    if APPROXIMATE {
        let [a, b, c, d] = values;
        let sum = (a + b) + (c + d);
        values.map(|x| (2.0 * x - sum) * regeneration)
    } else {
        MatrixKernel::matrixverb_sample(values, Parameters { regeneration })
    }
}

impl Processor {
    pub fn reset(&mut self) {
        self.state.reset();
    }

    #[inline(always)]
    fn sample<const APPROXIMATE: bool, const ACCELERATED: bool>(
        &mut self,
        input: [f64; 2],
        tuning: &Tuning,
    ) -> [f64; 2] {
        if ACCELERATED {
            self.sample_simd::<APPROXIMATE>(input, tuning)
        } else {
            self.sample_scalar::<APPROXIMATE, false>(input, tuning)
        }
    }

    #[inline(always)]
    fn sample_simd<const APPROXIMATE: bool>(
        &mut self,
        input: [f64; 2],
        tuning: &Tuning,
    ) -> [f64; 2] {
        let state = &mut self.state;
        let input = f64x2::new(input);
        let dry = input.abs().cmp_lt(splat64(1.18e-23)).blend(
            f64x2::new(state.random.map(|x| f64::from(x) * 1.18e-17)),
            input,
        );
        state.delays[12][state.cursors[12]] = dry.to_array();
        state.cursors[12] = next(state.cursors[12], tuning.delays[12]);
        let input = f64x2::new(state.delays[12][state.cursors[12]]);
        let input = sine_simd::<APPROXIMATE>(
            tuning.filters[0].sample_simd(input, &mut state.filters[0]) * splat64(tuning.wet),
        );
        let mut allpass = [splat64(0.0); 4];
        allpass
            .iter_mut()
            .zip(&mut state.delays[8..12])
            .zip(&mut state.cursors[8..12])
            .zip(&tuning.delays[8..12])
            .for_each(|(((output, delay), cursor), &length)| {
                let following = next(*cursor, length);
                let delayed = f64x2::new(delay[following]);
                let write = fma64(-delayed, splat64(0.5), input);
                delay[*cursor] = write.to_array();
                *cursor = following;
                *output = fma64(write, splat64(0.5), delayed);
            });
        let feeds = [
            allpass[3], allpass[2], allpass[1], allpass[0], allpass[0], allpass[1], allpass[2],
            allpass[3],
        ];
        let mut taps = [splat64(0.0); 8];
        state.delays[..8]
            .iter_mut()
            .zip(&mut state.cursors[..8])
            .zip(&mut state.phases)
            .zip(&state.feedback)
            .zip(feeds)
            .zip(&tuning.delays[..8])
            .zip(RATES)
            .zip(&mut taps)
            .for_each(
                |(((((((delay, cursor), phase), feedback), feed), &length), rate), tap)| {
                    delay[*cursor] = (feed + f64x2::new(*feedback)).to_array();
                    *cursor = next(*cursor, length);
                    *phase = (f64x2::new(*phase) + splat64(rate * tuning.speed)).to_array();
                    let offset = (sine_simd::<APPROXIMATE>(f64x2::new(*phase)) + splat64(1.0))
                        * splat64(tuning.depth);
                    let positions = offset.to_array();
                    let indices = positions.map(|offset| (*cursor as f64 + offset) as usize);
                    let first = f64x2::new([
                        delay[wrap(indices[0], length)][0],
                        delay[wrap(indices[1], length)][1],
                    ]);
                    let second = f64x2::new([
                        delay[wrap(indices[0] + 1, length)][0],
                        delay[wrap(indices[1] + 1, length)][1],
                    ]);
                    let fraction = offset - offset.floor();
                    let interpolated = fma64(second, fraction, first * (splat64(1.0) - fraction));
                    *tap = fma64(
                        splat64(1.0 - tuning.blend),
                        interpolated,
                        first * splat64(tuning.blend),
                    );
                },
            );
        let cross = splat64(tuning.crossmod);
        let retain = splat64(1.0 - tuning.crossmod.abs());
        taps[0] = fma64(taps[0], retain, taps[4] * cross);
        taps[4] = fma64(taps[4], retain, taps[0] * cross);
        let regeneration = splat64(tuning.regeneration);
        taps.chunks_exact(4)
            .zip(state.feedback.chunks_exact_mut(4))
            .for_each(|(taps, feedback)| {
                let [a, b, c, d] = [taps[0], taps[1], taps[2], taps[3]];
                let values = if APPROXIMATE {
                    let sum = (a + b) + (c + d);
                    [a, b, c, d].map(|x| (splat64(2.0) * x - sum) * regeneration)
                } else {
                    [
                        (a - ((b + c) + d)) * regeneration,
                        (b - ((a + c) + d)) * regeneration,
                        (c - ((a + b) + d)) * regeneration,
                        (d - ((a + b) + c)) * regeneration,
                    ]
                };
                feedback
                    .iter_mut()
                    .zip(values)
                    .for_each(|(feedback, value)| *feedback = value.to_array());
            });
        let [a, b, c, d, e, f, g, h] = taps;
        let sum = if APPROXIMATE {
            ((a + b) + (c + d)) + ((e + f) + (g + h))
        } else {
            a + b + c + d + e + f + g + h
        };
        let filtered = tuning.filters[1].sample_simd(sum * splat64(0.125), &mut state.filters[1]);
        let decoded = arcsine_simd::<APPROXIMATE>(filtered.max(splat64(-1.0)).min(splat64(1.0)));
        let wet = tuning.filters[2].sample_simd(decoded, &mut state.filters[2]);
        fma64(dry, splat64(1.0 - tuning.wet), wet).to_array()
    }

    #[inline(always)]
    fn sample_scalar<const APPROXIMATE: bool, const ACCELERATED: bool>(
        &mut self,
        input: [f64; 2],
        tuning: &Tuning,
    ) -> [f64; 2] {
        let state = &mut self.state;
        let dry = std::array::from_fn::<_, 2, _>(|channel| {
            select(
                input[channel].abs() < 1.18e-23,
                f64::from(state.random[channel]) * 1.18e-17,
                input[channel],
            )
        });
        state.delays[12][state.cursors[12]] = dry;
        state.cursors[12] = next(state.cursors[12], tuning.delays[12]);
        let input = state.delays[12][state.cursors[12]];
        let input = std::array::from_fn::<_, 2, _>(|channel| {
            sine::<APPROXIMATE, ACCELERATED>(
                tuning.filters[0]
                    .sample::<ACCELERATED>(input[channel], &mut state.filters[0][channel])
                    * tuning.wet,
            )
        });
        let mut allpass = [[0.0; 2]; 4];
        allpass
            .iter_mut()
            .zip(state.delays[8..12].iter_mut())
            .zip(state.cursors[8..12].iter_mut())
            .zip(&tuning.delays[8..12])
            .for_each(|(((output, delay), cursor), &length)| {
                let following = next(*cursor, length);
                let write = std::array::from_fn::<_, 2, _>(|channel| {
                    multiply_add::<ACCELERATED>(-delay[following][channel], 0.5, input[channel])
                });
                delay[*cursor] = write;
                *cursor = following;
                *output = std::array::from_fn(|channel| {
                    multiply_add::<ACCELERATED>(write[channel], 0.5, delay[following][channel])
                });
            });
        let feeds = [
            allpass[3], allpass[2], allpass[1], allpass[0], allpass[0], allpass[1], allpass[2],
            allpass[3],
        ];
        let mut taps = [[0.0; 2]; 8];
        state.delays[..8]
            .iter_mut()
            .zip(&mut state.cursors[..8])
            .zip(&mut state.phases)
            .zip(&state.feedback)
            .zip(feeds)
            .zip(&tuning.delays[..8])
            .zip(RATES)
            .zip(&mut taps)
            .for_each(
                |(((((((delay, cursor), phase), feedback), feed), &length), rate), tap)| {
                    delay[*cursor] = [feed[0] + feedback[0], feed[1] + feedback[1]];
                    *cursor = next(*cursor, length);
                    phase
                        .iter_mut()
                        .zip(tap)
                        .enumerate()
                        .for_each(|(channel, (phase, tap))| {
                            *phase += rate * tuning.speed;
                            let offset =
                                (sine::<APPROXIMATE, ACCELERATED>(*phase) + 1.0) * tuning.depth;
                            let working = (*cursor as f64 + offset) as usize;
                            let fraction = offset - offset.floor();
                            let first = delay[wrap(working, length)][channel];
                            let second = delay[wrap(working + 1, length)][channel];
                            let interpolated = multiply_add::<ACCELERATED>(
                                second,
                                fraction,
                                first * (1.0 - fraction),
                            );
                            *tap = multiply_add::<ACCELERATED>(
                                1.0 - tuning.blend,
                                interpolated,
                                first * tuning.blend,
                            );
                        });
                },
            );
        (0..2).for_each(|channel| {
            taps[0][channel] = multiply_add::<ACCELERATED>(
                taps[0][channel],
                1.0 - tuning.crossmod.abs(),
                taps[4][channel] * tuning.crossmod,
            );
            // Upstream intentionally uses the already-updated A tap to calculate E.
            taps[4][channel] = multiply_add::<ACCELERATED>(
                taps[4][channel],
                1.0 - tuning.crossmod.abs(),
                taps[0][channel] * tuning.crossmod,
            );
            let first = matrix::<APPROXIMATE>(
                [
                    taps[0][channel],
                    taps[1][channel],
                    taps[2][channel],
                    taps[3][channel],
                ],
                tuning.regeneration,
            );
            let second = matrix::<APPROXIMATE>(
                [
                    taps[4][channel],
                    taps[5][channel],
                    taps[6][channel],
                    taps[7][channel],
                ],
                tuning.regeneration,
            );
            state
                .feedback
                .iter_mut()
                .zip(first.into_iter().chain(second))
                .for_each(|(feedback, value)| {
                    feedback[channel] = value;
                });
        });
        std::array::from_fn(|channel| {
            let [a, b, c, d, e, f, g, h] = taps.map(|tap| tap[channel]);
            let sum = if APPROXIMATE {
                ((a + b) + (c + d)) + ((e + f) + (g + h))
            } else {
                a + b + c + d + e + f + g + h
            };
            let filtered =
                tuning.filters[1].sample::<ACCELERATED>(sum / 8.0, &mut state.filters[1][channel]);
            let decoded = arcsine::<APPROXIMATE, ACCELERATED>(filtered.clamp(-1.0, 1.0));
            let wet =
                tuning.filters[2].sample::<ACCELERATED>(decoded, &mut state.filters[2][channel]);
            multiply_add::<ACCELERATED>(dry[channel], 1.0 - tuning.wet, wet)
        })
    }

    pub fn process<const APPROXIMATE: bool, const ACCELERATED: bool>(
        &mut self,
        input: &[[f32; 2]],
        output: &mut [[f32; 2]],
        tuning: &Tuning,
    ) {
        assert_eq!(input.len(), output.len());
        input.iter().zip(output).for_each(|(input, output)| {
            let value = self.sample::<APPROXIMATE, ACCELERATED>(input.map(f64::from), tuning);
            *output = std::array::from_fn(|channel| {
                dither(value[channel], xorshift(&mut self.state.random[channel]))
            });
        });
    }

    pub fn process_f64<const APPROXIMATE: bool, const ACCELERATED: bool>(
        &mut self,
        input: &[[f64; 2]],
        output: &mut [[f64; 2]],
        tuning: &Tuning,
    ) {
        assert_eq!(input.len(), output.len());
        input.iter().zip(output).for_each(|(&input, output)| {
            *output = self.sample::<APPROXIMATE, ACCELERATED>(input, tuning);
            self.state.random.iter_mut().for_each(|random| {
                xorshift(random);
            });
        });
    }
}

pub struct DspCore {
    original: Processor,
    approximate: Processor,
    sample_rate: f64,
    mode_mix: f64,
    random: [u32; 2],
}

impl Default for DspCore {
    fn default() -> Self {
        Self {
            original: Processor::default(),
            approximate: Processor::default(),
            sample_rate: 44100.0,
            mode_mix: 0.0,
            random: super::state::SEEDS,
        }
    }
}

impl DspCore {
    pub fn init(&mut self, sample_rate: f64, original_order: bool) -> Result<(), &'static str> {
        Tuning::new(ProcessingParameters::default(), sample_rate)?;
        self.sample_rate = sample_rate;
        self.reset(original_order);
        Ok(())
    }

    pub fn reset(&mut self, original_order: bool) {
        self.original.reset();
        self.approximate.reset();
        self.mode_mix = f64::from(!original_order);
        self.random = super::state::SEEDS;
    }

    pub fn process(
        &mut self,
        input: &[[f32; 2]],
        output: &mut [[f32; 2]],
        parameters: ProcessingParameters,
        original_order: bool,
        accelerated: bool,
    ) -> Result<(), &'static str> {
        assert_eq!(input.len(), output.len());
        let tuning = Tuning::new(parameters, self.sample_rate)?;
        if accelerated {
            self.process_tuned::<true>(input, output, &tuning, original_order);
        } else {
            self.process_tuned::<false>(input, output, &tuning, original_order);
        }
        Ok(())
    }

    fn process_tuned<const ACCELERATED: bool>(
        &mut self,
        input: &[[f32; 2]],
        output: &mut [[f32; 2]],
        tuning: &Tuning,
        original_order: bool,
    ) {
        let target = f64::from(!original_order);
        let step = 1.0 / (self.sample_rate * 0.05);
        input.iter().zip(output).for_each(|(input, output)| {
            let original = self
                .original
                .sample::<false, ACCELERATED>(input.map(f64::from), tuning);
            let approximate = self
                .approximate
                .sample::<true, ACCELERATED>(input.map(f64::from), tuning);
            self.mode_mix += (target - self.mode_mix).clamp(-step, step);
            *output = std::array::from_fn(|channel| {
                let mixed = original[channel] * (1.0 - self.mode_mix)
                    + approximate[channel] * self.mode_mix;
                let random = xorshift(&mut self.random[channel]);
                self.original.state.random[channel] = random;
                self.approximate.state.random[channel] = random;
                dither(mixed, random)
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simd_matches_independent_scalar_fused_path_with_asymmetric_channels_and_automation() {
        for approximate in [false, true] {
            let mut simd = Processor::default();
            let mut scalar = Processor::default();
            for block in 0..256 {
                let parameters = ProcessingParameters {
                    vibrato: 1.0,
                    speed: 1.0,
                    room_size: [1.0, 0.0, 0.5, 0.17][block / 64],
                    flavor: [0.0, 1.0, 0.5, 0.13][block / 64],
                    filter: 0.37,
                    wet: 0.71,
                    ..ProcessingParameters::default()
                };
                let tuning = Tuning::new(parameters, 44100.0).unwrap();
                (0..256).for_each(|frame| {
                    let position = block * 256 + frame;
                    let input = [
                        ((position * 17 % 97) as f64 - 48.0) * 0.001,
                        ((position * 29 % 97) as f64 - 48.0) * 0.0007,
                    ];
                    let (actual, expected) = if approximate {
                        (
                            simd.sample_simd::<true>(input, &tuning),
                            scalar.sample_scalar::<true, true>(input, &tuning),
                        )
                    } else {
                        (
                            simd.sample_simd::<false>(input, &tuning),
                            scalar.sample_scalar::<false, true>(input, &tuning),
                        )
                    };
                    actual.into_iter().zip(expected).for_each(|(a, b)| {
                        assert!(a.is_finite() && b.is_finite());
                        assert!(
                            (a - b).abs() < 1e-10,
                            "approximate={approximate}, position={position}, SIMD={a}, scalar={b}"
                        );
                    });
                    simd.state.random.iter_mut().for_each(|random| {
                        xorshift(random);
                    });
                    scalar.state.random.iter_mut().for_each(|random| {
                        xorshift(random);
                    });
                });
            }
        }
    }

    #[test]
    fn crossfade_is_fifty_ms_and_reverses_without_resetting_live_histories() {
        let mut core = DspCore::default();
        core.init(48000.0, true).unwrap();
        let input = [[0.01, -0.02]; 1200];
        let mut output = [[0.0; 2]; 1200];
        let parameters = ProcessingParameters::default();
        core.process(&input, &mut output, parameters, true, false)
            .unwrap();
        // Fill the predelay/allpasses/network before checking live feedback.
        (0..8).for_each(|_| {
            core.process(&input, &mut output, parameters, true, false)
                .unwrap();
        });
        core.process(&input, &mut output, parameters, false, false)
            .unwrap();
        assert!((core.mode_mix - 0.5).abs() < 1e-12);
        assert!(core
            .original
            .state
            .feedback
            .iter()
            .flatten()
            .any(|x| x.abs() > 1e-10));
        assert!(core
            .approximate
            .state
            .feedback
            .iter()
            .flatten()
            .any(|x| x.abs() > 1e-10));
        core.process(&input, &mut output, parameters, true, true)
            .unwrap();
        assert!(core.mode_mix < 1e-12);
        core.process(&input, &mut output, parameters, false, true)
            .unwrap();
        core.process(&input, &mut output, parameters, false, true)
            .unwrap();
        assert!((core.mode_mix - 1.0).abs() < 1e-12);
        assert_eq!(core.original.state.cursors, core.approximate.state.cursors);
        assert_ne!(core.original.state.cursors, [1; 13]);
    }

    #[test]
    fn every_toggle_transition_matches_continuously_running_specialized_backends() {
        let modes = [(true, false), (false, false), (true, true), (false, true)];
        for (start_original, start_accelerated) in modes {
            for (end_original, end_accelerated) in modes {
                let mut core = DspCore::default();
                core.init(48000.0, start_original).unwrap();
                let mut original = Processor::default();
                let mut approximate = Processor::default();
                let tuning = Tuning::new(ProcessingParameters::default(), 48000.0).unwrap();
                let mut mix = f64::from(!start_original);
                let mut random = crate::dsp::state::SEEDS;
                (0..256).for_each(|block| {
                    let (original_mode, accelerated) = if block < 192 {
                        (start_original, start_accelerated)
                    } else {
                        (end_original, end_accelerated)
                    };
                    let input: [[f32; 2]; 64] = std::array::from_fn(|frame| {
                        let position = block * 64 + frame;
                        [
                            ((position * 17 % 97) as f32 - 48.0) * 0.001,
                            ((position * 29 % 97) as f32 - 48.0) * 0.001,
                        ]
                    });
                    let mut actual = [[0.0; 2]; 64];
                    core.process(&input, &mut actual, ProcessingParameters::default(), original_mode, accelerated).unwrap();
                    input.iter().zip(actual).for_each(|(input, actual)| {
                        let input = input.map(f64::from);
                        let (a, b) = if accelerated {
                            (original.sample::<false, true>(input, &tuning),
                             approximate.sample::<true, true>(input, &tuning))
                        } else {
                            (original.sample::<false, false>(input, &tuning),
                             approximate.sample::<true, false>(input, &tuning))
                        };
                        let target = f64::from(!original_mode);
                        let step = 1.0 / 2400.0;
                        mix += (target - mix).clamp(-step, step);
                        let expected: [f32; 2] = std::array::from_fn(|channel| {
                            let next = xorshift(&mut random[channel]);
                            original.state.random[channel] = next;
                            approximate.state.random[channel] = next;
                            dither(a[channel] * (1.0 - mix) + b[channel] * mix, next)
                        });
                        assert_eq!(actual, expected,
                            "transition ({start_original}, {start_accelerated}) -> ({end_original}, {end_accelerated}), block {block}");
                    });
                });
            }
        }
    }
}
