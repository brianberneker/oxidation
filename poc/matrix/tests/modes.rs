use oxidation_dsp::{
    dsp::core::{ApproximateNode, BorrowedNode, MatrixMath, Node},
    parameters::NodeParameters,
};

fn process<M: MatrixMath + Default>(
    input: &[f32],
    parameters: NodeParameters,
    accelerated: bool,
    block: usize,
) -> Vec<[f32; 8]> {
    let mut node = Node::<M>::default();
    let mut output = vec![[0.0; 8]; input.len()];
    input
        .chunks(block)
        .zip(output.chunks_mut(block))
        .for_each(|(input, output)| {
            node.process_with_acceleration(input, output, parameters, accelerated);
        });
    output
}

fn render(
    input: &[f32],
    parameters: NodeParameters,
    original: bool,
    accelerated: bool,
    block: usize,
) -> Vec<[f32; 8]> {
    use oxidation_dsp::dsp::core::{Balanced, OriginalOrder};
    if original {
        process::<OriginalOrder>(input, parameters, accelerated, block)
    } else {
        process::<Balanced>(input, parameters, accelerated, block)
    }
}

#[test]
fn all_four_modes_are_block_invariant_and_meet_error_budgets_across_tone_settings() {
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
    let modes = [(true, false), (false, false), (true, true), (false, true)];
    let mut differences = [[false; 4]; 4];
    for step in [1, 5, 9] {
        for high in 0..=4 {
            for low in 0..=4 {
                let parameters = NodeParameters::new(step, high, low).unwrap();
                let outputs = modes.map(|(original, accelerated)| {
                    render(&input, parameters, original, accelerated, 256)
                });
                for (index, ((original, accelerated), output)) in
                    modes.iter().zip(&outputs).enumerate()
                {
                    let mut peak = 0.0_f64;
                    let mut squared = 0.0;
                    output
                        .iter()
                        .flatten()
                        .zip(outputs[0].iter().flatten())
                        .for_each(|(&a, &b)| {
                            assert!(a.is_finite() && b.is_finite());
                            let error = (f64::from(a) - f64::from(b)).abs();
                            peak = peak.max(error);
                            squared += error * error;
                        });
                    let rms = (squared / (output.len() * 8) as f64).sqrt();
                    assert!(peak < 1e-5 && rms < 1e-6,
                        "mode {index}, damping {step}, high {high}, low {low}: peak {peak}, RMS {rms}");
                    // Irregular blocks cross all ring boundaries without multiplying every corpus run.
                    if high == 0 && low == 0 {
                        for block in [1, 47, 1024] {
                            assert_eq!(
                                &render(&input, parameters, *original, *accelerated, block),
                                output,
                                "mode {index}, damping {step}, block {block}"
                            );
                        }
                    }
                    outputs
                        .iter()
                        .enumerate()
                        .for_each(|(other, other_output)| {
                            differences[index][other] |= output != other_output;
                        });
                }
            }
        }
    }
    differences.iter().enumerate().for_each(|(a, differences)| {
        differences
            .iter()
            .enumerate()
            .filter(|(b, _)| a != *b)
            .for_each(|(b, &different)| {
                assert!(
                    different,
                    "modes {} and {} never produced distinct rounding",
                    a, b
                );
            });
    });
}

#[test]
fn all_four_modes_have_finite_ten_second_tails() {
    let parameters = NodeParameters::new(1, 0, 0).unwrap();
    let mut original = BorrowedNode::default();
    let mut approximate = ApproximateNode::default();
    let mut accelerated_original = BorrowedNode::default();
    let mut accelerated_approximate = ApproximateNode::default();
    let mut input = [0.0; 64];
    let mut output = [[0.0; 8]; 64];
    input[0] = 1.0;
    (0..7500).for_each(|_| {
        original.process_with_acceleration(&input, &mut output, parameters, false);
        assert!(output.iter().flatten().all(|sample| sample.is_finite()));
        approximate.process_with_acceleration(&input, &mut output, parameters, false);
        assert!(output.iter().flatten().all(|sample| sample.is_finite()));
        accelerated_original.process_with_acceleration(&input, &mut output, parameters, true);
        assert!(output.iter().flatten().all(|sample| sample.is_finite()));
        accelerated_approximate.process_with_acceleration(&input, &mut output, parameters, true);
        assert!(output.iter().flatten().all(|sample| sample.is_finite()));
        input[0] = 0.0;
    });
}

#[test]
fn acceleration_and_filter_automation_preserve_register_resident_histories() {
    fn check<M: MatrixMath + Default>() {
        let mut blocks = Node::<M>::default();
        let mut samples = Node::<M>::default();
        let input: [f32; 257] = std::array::from_fn(|i| ((i * 17 % 97) as f32 - 48.0) * 0.0001);
        for block in 0..96 {
            let parameters =
                NodeParameters::new(1 + block % 9, block % 5, (block / 5) % 5).unwrap();
            let accelerated = block % 4 < 2;
            let mut actual = [[0.0; 8]; 257];
            let mut expected = [[0.0; 8]; 257];
            blocks.process_with_acceleration(&input, &mut actual, parameters, accelerated);
            blocks.process_with_acceleration(&[], &mut [], parameters, !accelerated);
            input.iter().zip(&mut expected).for_each(|(input, output)| {
                samples.process_with_acceleration(
                    std::slice::from_ref(input),
                    std::slice::from_mut(output),
                    parameters,
                    accelerated,
                );
            });
            assert_eq!(actual, expected, "block {block}, accelerated={accelerated}");
        }
    }
    check::<oxidation_dsp::dsp::core::OriginalOrder>();
    check::<oxidation_dsp::dsp::core::Balanced>();
}
