use oxidation_dsp::{
    dsp::core::{BorrowedNode, OriginalNode},
    parameters::NodeParameters,
};

fn signal(length: usize) -> Vec<f32> {
    let mut seed = 0x7abeca01_u32;
    (0..length)
        .map(|sample| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let noise = (seed as i32 as f32) * (0.01 / i32::MAX as f32);
            match sample {
                0 => 1.0,
                1..=9999 => noise,
                _ => 0.0,
            }
        })
        .collect()
}

#[test]
fn all_tone_settings_and_damping_extremes_match_bitwise() {
    let input = signal(20000);
    let mut original = OriginalNode::default();
    let mut borrowed = BorrowedNode::default();
    let mut first = vec![[0.0; 8]; input.len()];
    let mut second = vec![[0.0; 8]; input.len()];
    for step in [1, 5, 9] {
        for unsolid in 0..=4 {
            for unreflect in 0..=4 {
                original.reset();
                borrowed.reset();
                let parameters = NodeParameters::new(step, unsolid, unreflect).unwrap();
                original.process(&input, &mut first, parameters);
                borrowed.process(&input, &mut second, parameters);
                first
                    .iter()
                    .zip(&second)
                    .enumerate()
                    .for_each(|(sample, (a, b))| {
                        assert_eq!(
                        a.map(f32::to_bits), b.map(f32::to_bits),
                        "sample {sample}, step {step}, unsolid {unsolid}, unreflect {unreflect}"
                    );
                    });
            }
        }
    }
}

#[test]
fn blocks_and_parameter_changes_preserve_state() {
    let input = signal(100000);
    let mut original = OriginalNode::default();
    let mut borrowed = BorrowedNode::default();
    let mut first = vec![[0.0; 8]; input.len()];
    let mut second = vec![[0.0; 8]; input.len()];
    for block in [1, 16, 21, 46, 47, 48, 64, 256, 1024] {
        original.reset();
        borrowed.reset();
        input
            .chunks(block)
            .zip(first.chunks_mut(block))
            .zip(second.chunks_mut(block))
            .enumerate()
            .for_each(|(index, ((input, first), second))| {
                let parameters = NodeParameters::new(
                    (index % 9 + 1) as u8,
                    (index % 5) as u8,
                    ((index / 5) % 5) as u8,
                )
                .unwrap();
                original.process(input, first, parameters);
                borrowed.process(input, second, parameters);
            });
        first
            .iter()
            .zip(&second)
            .enumerate()
            .for_each(|(sample, (a, b))| {
                assert_eq!(
                    a.map(f32::to_bits),
                    b.map(f32::to_bits),
                    "sample {sample}, block {block}"
                );
            });
    }
}

#[test]
fn impulse_tail_remains_finite_and_identical() {
    let mut input = vec![0.0; 480000];
    input[0] = 1.0;
    let mut original = OriginalNode::default();
    let mut borrowed = BorrowedNode::default();
    let mut first = vec![[0.0; 8]; input.len()];
    let mut second = vec![[0.0; 8]; input.len()];
    let parameters = NodeParameters::new(1, 0, 0).unwrap();
    original.process(&input, &mut first, parameters);
    borrowed.process(&input, &mut second, parameters);
    first.iter().zip(second).for_each(|(a, b)| {
        assert!(a.iter().all(|sample| sample.is_finite()));
        assert_eq!(a.map(f32::to_bits), b.map(f32::to_bits));
    });
    assert!(first.iter().flatten().any(|&sample| sample != 0.0));
}

#[test]
fn empty_blocks_do_not_change_state() {
    let parameters = NodeParameters::new(5, 0, 0).unwrap();
    let mut original = OriginalNode::default();
    let mut borrowed = BorrowedNode::default();
    original.process(&[], &mut [], parameters);
    borrowed.process(&[], &mut [], parameters);
    let input = signal(5000);
    let mut first = vec![[0.0; 8]; input.len()];
    let mut second = vec![[0.0; 8]; input.len()];
    original.process(&input, &mut first, parameters);
    borrowed.process(&input, &mut second, parameters);
    assert_eq!(first, second);
}

#[test]
fn rejects_invalid_parameters() {
    assert!(NodeParameters::new(10, 4, 4).is_err());
    assert!(NodeParameters::new(10, 4, 4).is_err());
    assert!(NodeParameters::new(5, 5, 4).is_err());
    assert!(NodeParameters::new(5, 4, 5).is_err());
    assert!(NodeParameters::new(0, 4, 4).is_ok());
}

#[test]
#[should_panic(expected = "assertion `left == right` failed")]
fn borrowed_rejects_mismatched_buffers() {
    BorrowedNode::default().process(&[0.0], &mut [], NodeParameters::new(5, 4, 4).unwrap());
}

#[test]
#[should_panic(expected = "assertion `left == right` failed")]
fn original_rejects_mismatched_buffers() {
    OriginalNode::default().process(&[0.0], &mut [], NodeParameters::new(5, 4, 4).unwrap());
}
