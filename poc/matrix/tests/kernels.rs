use oxidation_dsp::{dsp::core::DspCore, parameters::Parameters};

#[test]
fn matrixverb_known_values_and_block() {
    let input = [[1.0, 2.0, 4.0, 8.0], [0.0; 4]];
    let mut output = [[0.0; 4]; 2];
    DspCore::matrixverb(&input, &mut output, Parameters { regeneration: 0.5 });
    assert_eq!(output, [[-6.5, -5.5, -3.5, 0.5], [0.0; 4]]);
}

#[test]
fn ultralight_known_values_and_block() {
    let input = [[1.0, 2.0, 4.0, 8.0], [0.0; 4]];
    let mut output = [[0.0; 4]; 2];
    DspCore::ultralight3(&input, &mut output);
    assert_eq!(output, [[-13.0, -11.0, -7.0, 1.0], [0.0; 4]]);
}

#[test]
fn preserves_signed_zero() {
    let parameters = Parameters { regeneration: 0.5 };
    assert_eq!(
        DspCore::matrixverb_sample([-0.0; 4], parameters).map(f64::to_bits),
        [0; 4]
    );
    assert_eq!(
        DspCore::ultralight3_sample([-0.0; 4]).map(f32::to_bits),
        [0; 4]
    );
}

#[test]
fn cancellation_keeps_original_association() {
    let input = [1e20_f32, 1.0, -1e20, 1.0];
    let output = DspCore::ultralight3_sample(input);
    assert_eq!(output[1].to_bits(), 1.0_f32.to_bits());
    assert_eq!(output[3].to_bits(), 1.0_f32.to_bits());
}

#[test]
fn supports_empty_blocks() {
    DspCore::matrixverb(&[], &mut [], Parameters { regeneration: 0.5 });
    DspCore::ultralight3(&[], &mut []);
}

#[test]
#[should_panic(expected = "assertion `left == right` failed")]
fn matrixverb_rejects_mismatched_buffers() {
    DspCore::matrixverb(&[[0.0; 4]], &mut [], Parameters { regeneration: 0.5 });
}

#[test]
#[should_panic(expected = "assertion `left == right` failed")]
fn ultralight_rejects_mismatched_buffers() {
    DspCore::ultralight3(&[[0.0; 4]], &mut []);
}

#[test]
fn varying_block_lengths_match_single_sample_kernels() {
    let input64: Vec<_> = (0..4097)
        .map(|frame| {
            let x = frame as f64 * 0.013 - 0.5;
            [x, x * -0.317, x * 0.071, x * 0.891]
        })
        .collect();
    let input32: Vec<_> = input64
        .iter()
        .map(|frame| frame.map(|x| x as f32))
        .collect();
    [0, 1, 2, 3, 7, 16, 31, 64, 257, 4096, 4097]
        .into_iter()
        .for_each(|length| {
            let mut output64 = vec![[0.0; 4]; length];
            let mut output32 = vec![[0.0; 4]; length];
            let parameters = Parameters { regeneration: 0.47 };
            DspCore::matrixverb(&input64[..length], &mut output64, parameters);
            DspCore::ultralight3(&input32[..length], &mut output32);
            input64[..length]
                .iter()
                .zip(output64)
                .for_each(|(&input, output)| {
                    assert_eq!(
                        output.map(f64::to_bits),
                        DspCore::matrixverb_sample(input, parameters).map(f64::to_bits)
                    );
                });
            input32[..length]
                .iter()
                .zip(output32)
                .for_each(|(&input, output)| {
                    assert_eq!(
                        output.map(f32::to_bits),
                        DspCore::ultralight3_sample(input).map(f32::to_bits)
                    );
                });
        });
}

#[test]
fn feedback_matrix_chain_matches_direct_expressions() {
    let parameters = Parameters { regeneration: 0.47 };
    let mut candidate = [0.25, -0.5, 0.75, -1.0];
    let mut reference = candidate;
    let mut ultra_candidate = [0.25_f32, -0.5, 0.75, -1.0];
    let mut ultra_reference = ultra_candidate;
    (0..100_000).for_each(|step| {
        let drive = ((step % 97) as f64 - 48.0) * 1e-4;
        candidate[0] += drive;
        reference[0] += drive;
        candidate = DspCore::matrixverb_sample(candidate, parameters);
        let [a, b, c, d] = reference;
        reference = [
            (a - (b + c + d)) * parameters.regeneration,
            (b - (a + c + d)) * parameters.regeneration,
            (c - (a + b + d)) * parameters.regeneration,
            (d - (a + b + c)) * parameters.regeneration,
        ];
        assert_eq!(candidate.map(f64::to_bits), reference.map(f64::to_bits));

        ultra_candidate = ultra_candidate.map(|x| x * 0.47);
        ultra_reference = ultra_reference.map(|x| x * 0.47);
        ultra_candidate[0] += drive as f32;
        ultra_reference[0] += drive as f32;
        ultra_candidate = DspCore::ultralight3_sample(ultra_candidate);
        let [a, b, c, d] = ultra_reference;
        let sum = -(a + b + c + d);
        ultra_reference = [a * 2.0 + sum, b * 2.0 + sum, c * 2.0 + sum, d * 2.0 + sum];
        assert_eq!(
            ultra_candidate.map(f32::to_bits),
            ultra_reference.map(f32::to_bits)
        );
    });
}
