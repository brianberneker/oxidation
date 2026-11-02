#[inline(always)]
pub fn fast_abs(x: f32) -> f32 {
    f32::from_bits(x.to_bits() & 0x7FFFFFFF)
}
