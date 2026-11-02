use oxidation_dsp::acceleration::{f64x2, fma64, round64, splat64, CmpGt};

const INV_TAU: f64 = 1.0 / std::f64::consts::TAU;

#[inline(always)]
pub fn multiply_add<const ACCELERATED: bool>(a: f64, b: f64, c: f64) -> f64 {
    if ACCELERATED {
        a.mul_add(b, c)
    } else {
        a * b + c
    }
}

#[inline(always)]
pub fn select(condition: bool, yes: f64, no: f64) -> f64 {
    let mask = 0_u64.wrapping_sub(u64::from(condition));
    f64::from_bits((yes.to_bits() & mask) | (no.to_bits() & !mask))
}

#[inline(always)]
pub fn sine<const APPROXIMATE: bool, const ACCELERATED: bool>(value: f64) -> f64 {
    if !APPROXIMATE {
        return value.sin();
    }
    let x = value - (value * INV_TAU).round() * std::f64::consts::TAU;
    let x = x.abs().min(std::f64::consts::PI - x.abs()).copysign(x);
    let x2 = x * x;
    // Fold to [-pi/2, pi/2], then use Estrin grouping to shorten dependency chains.
    let p0 = multiply_add::<ACCELERATED>(1.0 / 120.0, x2, -1.0 / 6.0);
    let p1 = multiply_add::<ACCELERATED>(1.0 / 362880.0, x2, -1.0 / 5040.0);
    let p2 = multiply_add::<ACCELERATED>(1.0 / 6227020800.0, x2, -1.0 / 39916800.0);
    let x4 = x2 * x2;
    let low = multiply_add::<ACCELERATED>(p1, x4, p0);
    let high = multiply_add::<ACCELERATED>(-1.0 / 1307674368000.0, x4, p2);
    let p = multiply_add::<ACCELERATED>(high, x4 * x4, low);
    multiply_add::<ACCELERATED>(p * x2, x, x)
}

#[inline(always)]
pub fn arcsine<const APPROXIMATE: bool, const ACCELERATED: bool>(value: f64) -> f64 {
    if !APPROXIMATE {
        return value.asin();
    }
    let a = value.abs();
    let reduced = a > 0.5;
    let x = select(reduced, ((1.0 - a) * 0.5).sqrt(), a);
    let x2 = x * x;
    let p = multiply_add::<ACCELERATED>(12155.0 / 1245184.0, x2, 6435.0 / 557056.0);
    let p = multiply_add::<ACCELERATED>(p, x2, 143.0 / 10240.0);
    let p = multiply_add::<ACCELERATED>(p, x2, 231.0 / 13312.0);
    let p = multiply_add::<ACCELERATED>(p, x2, 63.0 / 2816.0);
    let p = multiply_add::<ACCELERATED>(p, x2, 35.0 / 1152.0);
    let p = multiply_add::<ACCELERATED>(p, x2, 5.0 / 112.0);
    let p = multiply_add::<ACCELERATED>(p, x2, 3.0 / 40.0);
    let p = multiply_add::<ACCELERATED>(p, x2, 1.0 / 6.0);
    let result = multiply_add::<ACCELERATED>(p * x2, x, x);
    select(reduced, std::f64::consts::FRAC_PI_2 - 2.0 * result, result).copysign(value)
}

#[inline(always)]
pub fn xorshift(value: &mut u32) -> u32 {
    *value ^= *value << 13;
    *value ^= *value >> 17;
    *value ^= *value << 5;
    *value
}

#[inline(always)]
pub fn dither(value: f64, random: u32) -> f32 {
    let float = value as f32;
    let bits = float.to_bits() & 0x7fff_ffff;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mask = 0_i32.wrapping_sub(i32::from(exponent == 0));
    let subnormal = (32 - bits.leading_zeros()) as i32 - 149;
    let exponent = ((subnormal & mask) | ((exponent - 126) & !mask)) * i32::from(bits != 0);
    let power = f64::from_bits(((exponent + 62 + 1023) as u64) << 52);
    (value + (f64::from(random) - 2147483647.0) * 5.5e-36 * power) as f32
}

#[inline(always)]
pub fn sine_simd<const APPROXIMATE: bool>(value: f64x2) -> f64x2 {
    if !APPROXIMATE {
        return f64x2::new(value.to_array().map(f64::sin));
    }
    let x = value - round64(value * splat64(INV_TAU)) * splat64(std::f64::consts::TAU);
    let sign = splat64(-0.0);
    let magnitude = x.abs().min(splat64(std::f64::consts::PI) - x.abs());
    let x = (magnitude & !sign) | (x & sign);
    let x2 = x * x;
    let p0 = fma64(splat64(1.0 / 120.0), x2, splat64(-1.0 / 6.0));
    let p1 = fma64(splat64(1.0 / 362880.0), x2, splat64(-1.0 / 5040.0));
    let p2 = fma64(splat64(1.0 / 6227020800.0), x2, splat64(-1.0 / 39916800.0));
    let x4 = x2 * x2;
    let low = fma64(p1, x4, p0);
    let high = fma64(splat64(-1.0 / 1307674368000.0), x4, p2);
    let p = fma64(high, x4 * x4, low);
    fma64(p * x2, x, x)
}

#[inline(always)]
pub fn arcsine_simd<const APPROXIMATE: bool>(value: f64x2) -> f64x2 {
    if !APPROXIMATE {
        return f64x2::new(value.to_array().map(f64::asin));
    }
    let a = value.abs();
    let reduced = a.cmp_gt(splat64(0.5));
    let x = reduced.blend(((splat64(1.0) - a) * splat64(0.5)).sqrt(), a);
    let x2 = x * x;
    let coefficients = [
        12155.0 / 1245184.0,
        6435.0 / 557056.0,
        143.0 / 10240.0,
        231.0 / 13312.0,
        63.0 / 2816.0,
        35.0 / 1152.0,
        5.0 / 112.0,
        3.0 / 40.0,
        1.0 / 6.0,
    ];
    let p = coefficients[1..]
        .iter()
        .fold(splat64(coefficients[0]), |p, &coefficient| {
            fma64(p, x2, splat64(coefficient))
        });
    let result = fma64(p * x2, x, x);
    let result = reduced.blend(
        splat64(std::f64::consts::FRAC_PI_2) - splat64(2.0) * result,
        result,
    );
    let sign = splat64(-0.0);
    (result & !sign) | (value & sign)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acceleration_switch_selects_fused_instead_of_unfused_arithmetic() {
        let a = std::hint::black_box(1.0 + 2.0_f64.powi(-27));
        let b = std::hint::black_box(1.0 - 2.0_f64.powi(-27));
        let c = std::hint::black_box(-1.0);
        assert_eq!(multiply_add::<false>(a, b, c), 0.0);
        assert_eq!(multiply_add::<true>(a, b, c), -2.0_f64.powi(-54));
    }

    #[test]
    fn polynomial_error_is_bounded_across_full_domains() {
        let mut sin_error = 0.0_f64;
        let mut asin_error = 0.0_f64;
        (0..=100_000).for_each(|i| {
            let x = f64::from(i) / 100_000.0;
            let phase = (x - 0.5) * 2000.0;
            let value = x * 2.0 - 1.0;
            sin_error = sin_error.max((sine::<true, false>(phase) - phase.sin()).abs());
            sin_error = sin_error.max((sine::<true, true>(phase) - phase.sin()).abs());
            asin_error = asin_error.max((arcsine::<true, false>(value) - value.asin()).abs());
            asin_error = asin_error.max((arcsine::<true, true>(value) - value.asin()).abs());
            for (lane, simd) in sine_simd::<true>(f64x2::new([phase, -phase * 0.37]))
                .to_array()
                .into_iter()
                .enumerate()
            {
                let angle = [phase, -phase * 0.37][lane];
                sin_error = sin_error.max((simd - angle.sin()).abs());
            }
            for (lane, simd) in arcsine_simd::<true>(f64x2::new([value, -value * 0.37]))
                .to_array()
                .into_iter()
                .enumerate()
            {
                let angle = [value, -value * 0.37][lane];
                asin_error = asin_error.max((simd - angle.asin()).abs());
            }
        });
        assert!(sin_error < 6e-10, "sine error {sin_error}");
        assert!(asin_error < 1.1e-8, "arcsine error {asin_error}");
    }

    #[test]
    fn sine_preserves_signed_zero_and_dither_scales_with_exponent() {
        assert_eq!(sine::<false, false>(-0.0).to_bits(), (-0.0_f64).to_bits());
        for value in [0.0, f64::from(f32::from_bits(1)), 0.001, -1.0, 100.0] {
            let actual = dither(value, 0xffff_ffff);
            assert!(actual.is_finite());
            assert!((f64::from(actual) - value).abs() <= value.abs() * 2e-7 + 6e-8);
        }
        assert_eq!(
            dither(0.0, u32::MAX),
            ((f64::from(u32::MAX) - 2147483647.0) * 5.5e-36 * 2.0_f64.powi(62)) as f32
        );
    }
}
