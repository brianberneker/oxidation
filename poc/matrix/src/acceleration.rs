pub use wide::{f32x4, f64x2, CmpGt, CmpLt};

// Explicit lane construction avoids wide's generic cast lowering to
// memset_pattern16 in the macOS ARM build with auto-vectorization disabled.
#[inline(always)]
pub fn splat64(value: f64) -> f64x2 {
    f64x2::new([value, value])
}

#[inline(always)]
pub fn splat32(value: f32) -> f32x4 {
    f32x4::new([value, value, value, value])
}

#[inline(always)]
pub fn round64(value: f64x2) -> f64x2 {
    #[cfg(target_arch = "aarch64")]
    {
        use std::arch::aarch64::*;
        let input = value.to_array();
        let mut output = [0.0; 2];
        // AArch64 guarantees NEON; round to nearest with ties to even.
        unsafe {
            vst1q_f64(output.as_mut_ptr(), vrndnq_f64(vld1q_f64(input.as_ptr())));
        }
        f64x2::new(output)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        f64x2::new(value.to_array().map(f64::round_ties_even))
    }
}

pub fn backend() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "NEON SIMD + hardware FMA"
    }
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("fma") {
            "SSE2 SIMD + hardware FMA"
        } else {
            "SSE2 SIMD + scalar fused fallback (CPU has no FMA)"
        }
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "portable vector operations + scalar fused fallback (unvalidated target)"
    }
}

#[inline(always)]
pub fn fma32(a: f32x4, b: f32x4, c: f32x4) -> f32x4 {
    #[cfg(target_arch = "aarch64")]
    {
        use std::arch::aarch64::*;
        let (a, b, c) = (a.to_array(), b.to_array(), c.to_array());
        let mut output = [0.0; 4];
        // AArch64 guarantees NEON/FMA; each load/store covers exactly four lanes.
        unsafe {
            vst1q_f32(
                output.as_mut_ptr(),
                vfmaq_f32(
                    vld1q_f32(c.as_ptr()),
                    vld1q_f32(a.as_ptr()),
                    vld1q_f32(b.as_ptr()),
                ),
            );
        }
        f32x4::new(output)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("fma") {
            // Runtime detection includes OS support for the FMA instruction's AVX state.
            return unsafe { x86_fma32(a, b, c) };
        }
        let (a, b, c) = (a.to_array(), b.to_array(), c.to_array());
        f32x4::new(std::array::from_fn(|lane| {
            a[lane].mul_add(b[lane], c[lane])
        }))
    }
}

#[inline(always)]
pub fn fma64(a: f64x2, b: f64x2, c: f64x2) -> f64x2 {
    #[cfg(target_arch = "aarch64")]
    {
        use std::arch::aarch64::*;
        let (a, b, c) = (a.to_array(), b.to_array(), c.to_array());
        let mut output = [0.0; 2];
        // AArch64 guarantees NEON/FMA; each load/store covers exactly two lanes.
        unsafe {
            vst1q_f64(
                output.as_mut_ptr(),
                vfmaq_f64(
                    vld1q_f64(c.as_ptr()),
                    vld1q_f64(a.as_ptr()),
                    vld1q_f64(b.as_ptr()),
                ),
            );
        }
        f64x2::new(output)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("fma") {
            // Called only after checking CPU and OS FMA support.
            return unsafe { x86_fma64(a, b, c) };
        }
        let (a, b, c) = (a.to_array(), b.to_array(), c.to_array());
        f64x2::new(std::array::from_fn(|lane| {
            a[lane].mul_add(b[lane], c[lane])
        }))
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "fma")]
unsafe fn x86_fma32(a: f32x4, b: f32x4, c: f32x4) -> f32x4 {
    use std::arch::x86_64::*;
    let (a, b, c) = (a.to_array(), b.to_array(), c.to_array());
    let mut output = [0.0; 4];
    // Unaligned loads/stores cover four lanes; the caller checks FMA support.
    unsafe {
        _mm_storeu_ps(
            output.as_mut_ptr(),
            _mm_fmadd_ps(
                _mm_loadu_ps(a.as_ptr()),
                _mm_loadu_ps(b.as_ptr()),
                _mm_loadu_ps(c.as_ptr()),
            ),
        );
    }
    f32x4::new(output)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "fma")]
unsafe fn x86_fma64(a: f64x2, b: f64x2, c: f64x2) -> f64x2 {
    use std::arch::x86_64::*;
    let (a, b, c) = (a.to_array(), b.to_array(), c.to_array());
    let mut output = [0.0; 2];
    // Unaligned loads/stores cover two lanes; the caller checks FMA support.
    unsafe {
        _mm_storeu_pd(
            output.as_mut_ptr(),
            _mm_fmadd_pd(
                _mm_loadu_pd(a.as_ptr()),
                _mm_loadu_pd(b.as_ptr()),
                _mm_loadu_pd(c.as_ptr()),
            ),
        );
    }
    f64x2::new(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splats_preserve_every_lane_bit_including_signed_zero_and_nan() {
        for bits in [0, 0x8000_0000, 0x3f80_0001, 0x7f80_0000, 0x7fc0_1234] {
            assert_eq!(
                splat32(f32::from_bits(bits)).to_array().map(f32::to_bits),
                [bits; 4]
            );
        }
        for bits in [
            0,
            0x8000_0000_0000_0000,
            0x3ff0_0000_0000_0001,
            0x7ff0_0000_0000_0000,
            0x7ff8_0000_0000_1234,
        ] {
            assert_eq!(
                splat64(f64::from_bits(bits)).to_array().map(f64::to_bits),
                [bits; 2]
            );
        }
    }

    #[test]
    fn packed_rounding_matches_nearest_even_for_ties_and_extremes() {
        for pair in [
            [0.0, -0.0],
            [0.5, -0.5],
            [1.5, -1.5],
            [2.5, -2.5],
            [1.499999999, -1.500000001],
            [f64::MAX, f64::MIN],
            [f64::INFINITY, f64::NEG_INFINITY],
        ] {
            assert_eq!(
                round64(f64x2::new(pair)).to_array().map(f64::to_bits),
                pair.map(|x| x.round_ties_even().to_bits())
            );
        }
        assert!(round64(splat64(f64::NAN))
            .to_array()
            .into_iter()
            .all(f64::is_nan));
    }

    #[test]
    fn packed_fma_is_fused_and_keeps_lanes_independent() {
        let a = 1.0 + 2.0_f32.powi(-13);
        let b = 1.0 - 2.0_f32.powi(-13);
        assert_eq!(
            fma32(
                f32x4::new([a, 2.0, -3.0, 7.0]),
                f32x4::new([b, 3.0, 5.0, -2.0]),
                f32x4::new([-1.0, 4.0, 1.0, 6.0])
            )
            .to_array(),
            [-2.0_f32.powi(-26), 10.0, -14.0, -8.0]
        );
        let a = 1.0 + 2.0_f64.powi(-27);
        let b = 1.0 - 2.0_f64.powi(-27);
        assert_eq!(
            fma64(
                f64x2::new([a, -3.0]),
                f64x2::new([b, 5.0]),
                f64x2::new([-1.0, 1.0])
            )
            .to_array(),
            [-2.0_f64.powi(-54), -14.0]
        );
    }
}
