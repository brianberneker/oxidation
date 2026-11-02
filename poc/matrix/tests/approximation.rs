use oxidation_dsp::{
    dsp::core::{ApproximateNode, Balanced, BorrowedNode, MatrixMath, OriginalOrder},
    parameters::NodeParameters,
};

#[test]
fn balanced_backend_is_a_real_change_in_association() {
    let input = [1e20, 1.0, -1e20, 1.0];
    assert_eq!(
        OriginalOrder::transform(input)[1].to_bits(),
        1.0_f32.to_bits()
    );
    assert_eq!(Balanced::transform(input)[1].to_bits(), 2.0_f32.to_bits());
}

#[test]
fn approximation_error_and_tail_remain_bounded() {
    let mut worst_peak = 0.0_f64;
    let mut worst_rms = 0.0_f64;
    for step in [1, 5, 9] {
        for high in 0..=4 {
            for low in 0..=4 {
                let mut seed = 0x7abeca01_u32;
                let input: Vec<_> = (0..48000)
                    .map(|sample| {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        match sample {
                            0 => 1.0,
                            1..=9999 => seed as i32 as f32 * (0.01 / i32::MAX as f32),
                            _ => 0.0,
                        }
                    })
                    .collect();
                let parameters = NodeParameters::new(step, high, low).unwrap();
                let mut original = BorrowedNode::default();
                let mut approximate = ApproximateNode::default();
                let mut first = vec![[0.0; 8]; input.len()];
                let mut second = vec![[0.0; 8]; input.len()];
                original.process(&input, &mut first, parameters);
                approximate.process(&input, &mut second, parameters);
                let mut squared = 0.0;
                first
                    .iter()
                    .flatten()
                    .zip(second.iter().flatten())
                    .for_each(|(&a, &b)| {
                        assert!(a.is_finite() && b.is_finite());
                        let error = (f64::from(a) - f64::from(b)).abs();
                        worst_peak = worst_peak.max(error);
                        squared += error * error;
                    });
                worst_rms = worst_rms.max((squared / (input.len() * 8) as f64).sqrt());
            }
        }
    }
    println!("balanced backend: worst peak {worst_peak:e}, worst RMS {worst_rms:e}");
    assert!(worst_peak > 0.0, "test must exercise different rounding");
    assert!(
        worst_peak < 1e-5,
        "peak error budget exceeded: {worst_peak:e}"
    );
    assert!(worst_rms < 1e-6, "RMS error budget exceeded: {worst_rms:e}");
}

#[test]
fn approximate_ten_second_impulse_tail_is_finite() {
    let parameters = NodeParameters::new(1, 0, 0).unwrap();
    let mut node = ApproximateNode::default();
    let mut input = [0.0; 64];
    let mut output = [[0.0; 8]; 64];
    input[0] = 1.0;
    node.process(&input, &mut output, parameters);
    input[0] = 0.0;
    (1..7500).for_each(|_| {
        node.process(&input, &mut output, parameters);
        assert!(output.iter().flatten().all(|sample| sample.is_finite()));
    });
}
