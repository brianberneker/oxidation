use super::state::{DspState, HOST_BLOCK};
use oxidation_dsp::acceleration::{f32x4, fma32, splat32};
use oxidation_dsp::parameters::NodeParameters;

const PRIME_COUNT: usize = 999;

const fn is_prime(value: usize) -> bool {
    let mut divisor = 2;
    while divisor * divisor <= value {
        if value % divisor == 0 {
            return false;
        }
        divisor += 1;
    }
    true
}

const fn make_primes() -> [usize; PRIME_COUNT] {
    let mut primes = [0; PRIME_COUNT];
    let mut found = 0;
    let mut candidate = 3;
    while found < PRIME_COUNT {
        if is_prime(candidate) {
            primes[found] = candidate;
            found += 1;
        }
        candidate += 2;
    }
    primes
}

const PRIMES: [usize; PRIME_COUNT] = make_primes();

#[derive(Clone, Copy)]
pub struct ProcessingParameters {
    pub node: NodeParameters,
    pub damping_bypass: bool,
    pub predelay: usize,
    pub wet: f32,
    pub front_back: f32,
    pub bottom_top: f32,
    pub distance: f32,
    pub original_order: bool,
    pub accelerated: bool,
}

#[derive(Clone, Copy)]
struct PreDelayTuning {
    q: usize,
    r: usize,
    s: usize,
    t: usize,
    bypass: bool,
}

impl PreDelayTuning {
    fn new(index: usize) -> Self {
        let mut scaled = index.min(PRIME_COUNT - 1) as f32;
        let t = PRIMES[scaled as usize];
        scaled = (scaled * 81.408).sqrt();
        let q = PRIMES[(scaled as usize).min(PRIME_COUNT - 1)];
        scaled = (scaled * 81.408).sqrt();
        let r = PRIMES[(scaled as usize).min(PRIME_COUNT - 1)];
        scaled = (scaled * 81.408).sqrt();
        let s = PRIMES[(scaled as usize).min(PRIME_COUNT - 1)];
        Self {
            q,
            r,
            s,
            t,
            bypass: index == 0,
        }
    }
}

pub struct DspCore {
    state: DspState,
    native_step: f32,
    fade_step: f32,
    sample_rate: f32,
}

#[inline(always)]
fn interpolate(a: f32, b: f32, amount: f32) -> f32 {
    a * (1.0 - amount) + b * amount
}

#[inline(always)]
fn next_cursor(cursor: usize, delay: usize) -> usize {
    let next = cursor + 1;
    next * usize::from(next <= delay)
}

#[inline(always)]
fn xorshift(value: &mut u32) -> u32 {
    *value ^= *value << 13;
    *value ^= *value >> 17;
    *value ^= *value << 5;
    *value
}

#[inline(always)]
fn frexp_exponent(value: f32) -> i32 {
    let exponent = ((value.to_bits() >> 23) & 0xff) as i32;
    if exponent == 0 {
        if value == 0.0 {
            0
        } else {
            let leading = (value.to_bits() & 0x7f_ffff).leading_zeros() as i32 - 8;
            -125 - leading
        }
    } else {
        exponent - 126
    }
}

#[inline(always)]
fn dither(value: f32, random: u32) -> f32 {
    let exponent = frexp_exponent(value);
    let centered = f64::from(random) - 2_147_483_647.0;
    let noise = centered * 3.553e-44_f64 * 2.0_f64.powi(exponent + 62);
    (f64::from(value) + noise) as f32
}

impl Default for DspCore {
    fn default() -> Self {
        Self::new()
    }
}

impl DspCore {
    pub fn new() -> Self {
        Self {
            state: DspState::default(),
            native_step: 1.0,
            fade_step: 1.0 / 2400.0,
            sample_rate: 48000.0,
        }
    }

    pub fn init(&mut self, sample_rate: f32, original_order: bool) -> Result<(), &'static str> {
        if !sample_rate.is_finite() || !(8000.0..=384000.0).contains(&sample_rate) {
            return Err("oxidation requires a finite sample rate between 8 kHz and 384 kHz");
        }
        self.sample_rate = sample_rate;
        self.native_step = 48000.0 / sample_rate;
        self.fade_step = 1.0 / (sample_rate * 0.05);
        self.reset(original_order);
        Ok(())
    }

    pub fn reset(&mut self, original_order: bool) {
        self.state.reset(original_order);
    }

    #[inline(always)]
    fn predelay_sample(
        state: &mut super::state::PreDelay,
        input: f32,
        tuning: PreDelayTuning,
        accelerated: bool,
    ) -> f32 {
        let [_, mut r_out, mut s_out, mut t_out] = state.outputs;
        let mut q_out;
        let [q_cursor, r_cursor, s_cursor, t_cursor] = &mut state.cursors;

        let q_next = next_cursor(*q_cursor, tuning.q);
        let r_next = next_cursor(*r_cursor, tuning.r);
        let s_next = next_cursor(*s_cursor, tuning.s);
        let t_next = next_cursor(*t_cursor, tuning.t);

        [q_out, r_out, s_out] = if accelerated {
            let delayed = f32x4::new([state.q[q_next], state.r[r_next], state.s[s_next], 0.0]);
            let feeds = f32x4::new([r_out, s_out, t_out, 0.0]);
            let result = fma32(-delayed, splat32(0.5), feeds).to_array();
            [result[0], result[1], result[2]]
        } else {
            [
                r_out - state.q[q_next] * 0.5,
                s_out - state.r[r_next] * 0.5,
                t_out - state.s[s_next] * 0.5,
            ]
        };
        t_out = input;

        state.q[*q_cursor] = q_out;
        state.r[*r_cursor] = r_out;
        state.s[*s_cursor] = s_out;
        state.t[*t_cursor] = t_out;

        *q_cursor = q_next;
        *r_cursor = r_next;
        *s_cursor = s_next;
        *t_cursor = t_next;

        [q_out, r_out, s_out] = if accelerated {
            let values = f32x4::new([q_out, r_out, s_out, 0.0]);
            let delayed = f32x4::new([
                state.q[*q_cursor],
                state.r[*r_cursor],
                state.s[*s_cursor],
                0.0,
            ]);
            let result = fma32(values, splat32(0.5), delayed).to_array();
            [result[0], result[1], result[2]]
        } else {
            [
                q_out * 0.5 + state.q[*q_cursor],
                r_out * 0.5 + state.r[*r_cursor],
                s_out * 0.5 + state.s[*s_cursor],
            ]
        };
        t_out = state.t[*t_cursor];
        state.outputs = [q_out, r_out, s_out, t_out];

        if tuning.bypass {
            input
        } else {
            q_out
        }
    }

    #[inline(always)]
    fn fold_output(output: [f32; 8], back: f32, ceiling: f32) -> [f32; 2] {
        let front = 1.0 - back;
        let floor = 1.0 - ceiling;
        [
            output[2] * back * floor
                + output[0] * floor * front
                + output[6] * back * ceiling
                + output[4] * ceiling * front,
            output[3] * back * floor
                + output[1] * floor * front
                + output[7] * back * ceiling
                + output[5] * ceiling * front,
        ]
    }

    #[inline(always)]
    fn native_output(
        output: [f32; 8],
        dry: [f32; 2],
        predelayed: f32,
        parameters: ProcessingParameters,
        accelerated: bool,
    ) -> [f32; 2] {
        let wet = if parameters.damping_bypass {
            [predelayed; 2]
        } else {
            Self::fold_output(output, parameters.front_back, parameters.bottom_top)
        };
        if accelerated {
            let result = fma32(
                f32x4::new([wet[0], wet[1], 0.0, 0.0]),
                splat32(parameters.wet),
                f32x4::new([dry[0], dry[1], 0.0, 0.0]) * splat32(1.0 - parameters.wet),
            )
            .to_array();
            [result[0], result[1]]
        } else {
            [
                wet[0] * parameters.wet + dry[0] * (1.0 - parameters.wet),
                wet[1] * parameters.wet + dry[1] * (1.0 - parameters.wet),
            ]
        }
    }

    #[inline(always)]
    fn distance_filter(input: f32, state: &mut [f32; 8], distance: f32, accelerated: bool) -> f32 {
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
        let input = input * 0.5;
        if accelerated {
            let weights = f32x4::new([distance * 0.875, distance, distance, distance]);
            let previous = f32x4::new([a, b, c, d]);
            let delta = f32x4::new([input, a, b, c]) - previous - f32x4::new([e, f, g, h]);
            [e, f, g, h] = fma32(weights, delta, f32x4::new([e, f, g, h])).to_array();
            [a, b, c, d] = fma32(weights, f32x4::new([e, f, g, h]), previous).to_array();
        } else {
            e = distance * 0.875 * (input - a - e) + e;
            f = distance * (a - b - f) + f;
            g = distance * (b - c - g) + g;
            h = distance * (c - d - h) + h;
            a = distance * 0.875 * e + a;
            b = distance * f + b;
            c = distance * g + c;
            d = distance * h + d;
        }
        *state = [a, b, c, d, e, f, g, h];
        c + d
    }

    #[inline(always)]
    pub fn process_block(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        parameters: ProcessingParameters,
    ) {
        assert_eq!(left.len(), right.len());
        assert!(left.len() <= HOST_BLOCK);

        let tuning = PreDelayTuning::new(parameters.predelay);
        let distance = (1.0 - parameters.distance).powf(self.sample_rate / 48000.0 + 1.0);
        let state = &mut self.state;
        let mut native_count = 0;

        left.iter()
            .zip(right.iter())
            .zip(&mut state.positions)
            .for_each(|((&left, &right), position)| {
                let current_left = if crate::utils::math::fast_abs(left) < 1.18e-23 {
                    (f64::from(xorshift(&mut state.denormal[0])) * 1.18e-17) as f32
                } else {
                    left
                };
                let current_right = if crate::utils::math::fast_abs(right) < 1.18e-23 {
                    (f64::from(xorshift(&mut state.denormal[1])) * 1.18e-17) as f32
                } else {
                    right
                };
                let current = [current_left, current_right];
                let start_phase = state.phase;
                let mut fraction = (1.0 - start_phase) / self.native_step;
                state.phase += self.native_step;
                while state.phase >= 1.0 {
                    let dry = [
                        interpolate(state.previous_input[0], current[0], fraction),
                        interpolate(state.previous_input[1], current[1], fraction),
                    ];
                    let mono = (dry[0] + dry[1]) * 0.5;
                    state.dry[native_count] = dry;
                    state.mono[native_count] = Self::predelay_sample(
                        &mut state.predelay,
                        mono,
                        tuning,
                        parameters.accelerated,
                    );
                    native_count += 1;
                    state.phase -= 1.0;
                    fraction += 1.0 / self.native_step;
                }
                state.previous_input = current;
                *position = (native_count, state.phase);
            });

        state.exact.process_with_acceleration(
            &state.mono[..native_count],
            &mut state.exact_output[..native_count],
            parameters.node,
            parameters.accelerated,
        );
        state.approximate.process_with_acceleration(
            &state.mono[..native_count],
            &mut state.approximate_output[..native_count],
            parameters.node,
            parameters.accelerated,
        );

        state.exact_output[..native_count]
            .iter()
            .zip(&state.dry[..native_count])
            .zip(&state.mono[..native_count])
            .zip(&mut state.exact_wet[2..])
            .for_each(|(((&output, &dry), &predelayed), wet)| {
                *wet = Self::native_output(
                    output,
                    dry,
                    predelayed,
                    parameters,
                    parameters.accelerated,
                );
            });
        state.approximate_output[..native_count]
            .iter()
            .zip(&state.dry[..native_count])
            .zip(&state.mono[..native_count])
            .zip(&mut state.approximate_wet[2..])
            .for_each(|(((&output, &dry), &predelayed), wet)| {
                *wet = Self::native_output(
                    output,
                    dry,
                    predelayed,
                    parameters,
                    parameters.accelerated,
                );
            });

        let target = f32::from(!parameters.original_order);
        left.iter_mut().zip(right).zip(&state.positions).for_each(
            |((left, right), &(index, phase))| {
                let exact = [
                    interpolate(
                        state.exact_wet[index][0],
                        state.exact_wet[index + 1][0],
                        phase,
                    ),
                    interpolate(
                        state.exact_wet[index][1],
                        state.exact_wet[index + 1][1],
                        phase,
                    ),
                ];
                let approximate = [
                    interpolate(
                        state.approximate_wet[index][0],
                        state.approximate_wet[index + 1][0],
                        phase,
                    ),
                    interpolate(
                        state.approximate_wet[index][1],
                        state.approximate_wet[index + 1][1],
                        phase,
                    ),
                ];
                state.mode_mix += (target - state.mode_mix).clamp(-self.fade_step, self.fade_step);
                let mix_left = interpolate(exact[0], approximate[0], state.mode_mix);
                let mix_right = interpolate(exact[1], approximate[1], state.mode_mix);
                let output_left = Self::distance_filter(
                    mix_left,
                    &mut state.distance_left,
                    distance,
                    parameters.accelerated,
                );
                let output_right = Self::distance_filter(
                    mix_right,
                    &mut state.distance_right,
                    distance,
                    parameters.accelerated,
                );

                let left_noise = xorshift(&mut state.dither[0]);
                let right_noise = xorshift(&mut state.dither[1]);
                if state.dither[0].wrapping_sub(state.dither[1]) < 1_073_741_824
                    || state.dither[1].wrapping_sub(state.dither[0]) < 1_073_741_824
                {
                    xorshift(&mut state.dither[1]);
                }
                *left = dither(output_left, left_noise);
                *right = dither(output_right, right_noise);
            },
        );

        state.exact_wet[0] = state.exact_wet[native_count];
        state.exact_wet[1] = state.exact_wet[native_count + 1];
        state.approximate_wet[0] = state.approximate_wet[native_count];
        state.approximate_wet[1] = state.approximate_wet[native_count + 1];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parameters(original_order: bool, accelerated: bool) -> ProcessingParameters {
        ProcessingParameters {
            node: NodeParameters::new(2, 4, 4).unwrap(),
            damping_bypass: false,
            predelay: 0,
            wet: 0.5,
            front_back: 0.0,
            bottom_top: 0.0,
            distance: 0.0,
            original_order,
            accelerated,
        }
    }

    fn render(sample_rate: f32, block: usize, mode: (bool, bool)) -> Vec<[f32; 2]> {
        let mut core = DspCore::new();
        core.init(sample_rate, mode.0).unwrap();
        let mut output = Vec::new();
        let length = 20000;
        let mut offset = 0;
        while offset < length {
            let count = block.min(length - offset);
            let mut left = vec![0.0; count];
            let mut right = vec![0.0; count];
            left.iter_mut()
                .zip(&mut right)
                .enumerate()
                .for_each(|(index, (l, r))| {
                    let sample = index + offset;
                    let input = match sample {
                        0 => 1.0,
                        1..=5000 => ((sample * 17 % 97) as f32 - 48.0) * 0.001,
                        _ => 0.0,
                    };
                    *l = input;
                    *r = input * 0.37;
                });
            core.process_block(&mut left, &mut right, parameters(mode.0, mode.1));
            output.extend(left.into_iter().zip(right).map(|(a, b)| [a, b]));
            offset += count;
        }
        output
    }

    #[test]
    fn supports_all_declared_sample_rates_without_nonfinite_output() {
        for rate in [
            8000.0, 11025.0, 22050.0, 44100.0, 48000.0, 88200.0, 96000.0, 192000.0, 384000.0,
        ] {
            for mode in [(true, false), (false, false), (true, true), (false, true)] {
                let output = render(rate, 64, mode);
                assert!(
                    output.iter().flatten().all(|sample| sample.is_finite()),
                    "rate {rate}, mode {mode:?}"
                );
                assert!(
                    output.iter().flatten().any(|&sample| sample != 0.0),
                    "rate {rate}, mode {mode:?}"
                );
            }
        }
    }

    #[test]
    fn resampling_is_invariant_to_host_block_boundaries() {
        for rate in [8000.0, 44100.0, 48000.0, 96000.0, 384000.0] {
            for mode in [(true, false), (false, false), (true, true), (false, true)] {
                let reference = render(rate, 64, mode);
                for block in [1, 7, 31, 47] {
                    let actual = render(rate, block, mode);
                    let mismatch = reference
                        .iter()
                        .zip(actual)
                        .position(|(a, b)| a.map(f32::to_bits) != b.map(f32::to_bits));
                    assert!(
                        mismatch.is_none(),
                        "rate {rate}, block {block}, mode {mode:?}: {mismatch:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn all_four_math_and_acceleration_modes_are_selectable() {
        let original = render(48000.0, 64, (true, false));
        let approximate = render(48000.0, 64, (false, false));
        let fused_original = render(48000.0, 64, (true, true));
        let fused_approximate = render(48000.0, 64, (false, true));
        assert_ne!(original, approximate);
        assert_ne!(original, fused_original);
        assert_ne!(approximate, fused_approximate);
        assert_ne!(original, fused_approximate);
        assert_ne!(approximate, fused_original);
        assert_ne!(fused_original, fused_approximate);
        for output in [&approximate, &fused_original, &fused_approximate] {
            let mut peak = 0.0_f64;
            let mut squared = 0.0;
            output
                .iter()
                .flatten()
                .zip(original.iter().flatten())
                .for_each(|(&a, &b)| {
                    assert!(a.is_finite());
                    let error = (f64::from(a) - f64::from(b)).abs();
                    peak = peak.max(error);
                    squared += error * error;
                });
            let rms = (squared / (output.len() * 2) as f64).sqrt();
            assert!(peak < 1e-5 && rms < 1e-6, "peak {peak}, RMS {rms}");
        }
    }

    #[test]
    fn all_four_integrated_modes_dispatch_both_delay_nodes_correctly() {
        use oxidation_dsp::dsp::core::{ApproximateNode, BorrowedNode};
        for mode in [(true, false), (false, false), (true, true), (false, true)] {
            let mut core = DspCore::new();
            core.init(48000.0, mode.0).unwrap();
            let mut original = BorrowedNode::default();
            let mut approximate = ApproximateNode::default();
            let parameters = parameters(mode.0, mode.1);
            (0..512).for_each(|block| {
                let mut left = std::array::from_fn::<_, HOST_BLOCK, _>(|frame| {
                    (((block * HOST_BLOCK + frame) * 17 % 97) as f32 - 48.0) * 0.001
                });
                let mut right = left.map(|x| x * 0.37);
                core.process_block(&mut left, &mut right, parameters);
                let mut expected_original = [[0.0; 8]; HOST_BLOCK];
                let mut expected_approximate = [[0.0; 8]; HOST_BLOCK];
                original.process_with_acceleration(
                    &core.state.mono[..HOST_BLOCK],
                    &mut expected_original,
                    parameters.node,
                    mode.1,
                );
                approximate.process_with_acceleration(
                    &core.state.mono[..HOST_BLOCK],
                    &mut expected_approximate,
                    parameters.node,
                    mode.1,
                );
                assert_eq!(
                    &core.state.exact_output[..HOST_BLOCK],
                    &expected_original,
                    "original node, mode {mode:?}, block {block}"
                );
                assert_eq!(
                    &core.state.approximate_output[..HOST_BLOCK],
                    &expected_approximate,
                    "approximate node, mode {mode:?}, block {block}"
                );
                assert_eq!(core.state.mode_mix, f32::from(!mode.0));
            });
        }
    }

    #[test]
    fn predelay_and_plugin_controls_change_the_result() {
        let render_parameters = |parameters: ProcessingParameters| {
            let mut core = DspCore::new();
            core.init(48000.0, true).unwrap();
            let mut rendered = Vec::new();
            (0..400).for_each(|block| {
                let mut left = [0.0; HOST_BLOCK];
                let mut right = [0.0; HOST_BLOCK];
                if block == 0 {
                    left[0] = 1.0;
                }
                core.process_block(&mut left, &mut right, parameters);
                rendered.extend(left.into_iter().zip(right).map(|(l, r)| [l, r]));
            });
            rendered
        };
        let mut modified = parameters(true, false);
        modified.predelay = 998;
        modified.wet = 1.0;
        modified.front_back = 0.7;
        modified.bottom_top = 0.6;
        modified.distance = 0.2;
        let baseline = render_parameters(parameters(true, false));
        let changed = render_parameters(modified);
        assert!(changed.iter().flatten().any(|&sample| sample != 0.0));
        assert_ne!(baseline, changed);
    }

    #[test]
    fn reset_reinitializes_tails_phase_and_toggles() {
        let mut core = DspCore::new();
        core.init(48000.0, true).unwrap();
        let mut left = [0.25; 64];
        let mut right = [-0.5; 64];
        core.process_block(&mut left, &mut right, parameters(false, false));
        core.reset(true);
        assert_eq!(core.state.mode_mix, 0.0);
        assert_eq!(core.state.phase, 0.0);
        assert_eq!(core.state.predelay.cursors, [1; 4]);
    }

    #[test]
    fn rejects_invalid_sample_rates() {
        let mut core = DspCore::new();
        for rate in [0.0, -1.0, 7999.0, 384001.0, f32::NAN, f32::INFINITY] {
            assert!(core.init(rate, true).is_err());
        }
    }
}
