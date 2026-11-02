use oxidation_dsp::{
    dsp::core::{BorrowedNode, OriginalNode},
    parameters::NodeParameters,
};

// FNV-1a/64 of little-endian output bytes from the external C++ oracle.
// Corpus provenance, ordering and SHA-256 are documented in README.md.
const EXPECTED: [u64; 75] = [
    0x82333a309fa8840e,
    0x7368686ea96bd253,
    0x7d4cca6b78fcb0b9,
    0x12bdbc307f673c2e,
    0x350c493c02aa744d,
    0x5e886f6e9104b8cf,
    0x71699eaf3abc6a39,
    0x20444c61749f57e1,
    0x05d55994c02310b2,
    0x069226d36de359de,
    0xb4f99868c7500105,
    0x1af58ba5ebf2b5c3,
    0x8e6cb7f4674011c1,
    0x51e60fbffd099486,
    0xd168dcc9e1d9e808,
    0x4050d7fb6dbb6d19,
    0x22823f522df71454,
    0x8c2b61ec676ba040,
    0xb29d785a808cdf29,
    0xb4e5bb1ff4134bc2,
    0x8f1f0bc27e4f393e,
    0x47486dcc8977940b,
    0x57fb7c5b6ff4ba58,
    0xa86590be3813fa3e,
    0x50625130a4ffa806,
    0x825ecad7a8053449,
    0x0efafe278a091b12,
    0x34f7fd2328670e8f,
    0x1afa36e9de0f2d2c,
    0x1365ef58aa782554,
    0xb6ee52b19eb8ab25,
    0x174a1104bb4b682d,
    0x39a6466b4994e733,
    0xacbc80860c693a43,
    0x49a88bbb2d139da3,
    0x537d8c41532124be,
    0xa098b8c3553fa524,
    0x440c99219a816cf9,
    0xb63fe49fd3ef16d0,
    0xf9ab51550b16780a,
    0x510d4822eb7dc17c,
    0xfcebe148f6db8da7,
    0x92ee8bda3f89eba2,
    0x4116de60ed262199,
    0x35c1fc7dfaaf35c3,
    0xafebcb808ae9fdee,
    0xca42997b82f43c18,
    0x0b6fbf033cd678eb,
    0x984c34da6c5afda1,
    0x33341fa73679738a,
    0x26cf2696ad8a727d,
    0x319d08fc6dd51460,
    0xe9a696033222478d,
    0x12b75675d8937460,
    0xda83da41f03b5bb5,
    0x47f40300d4ec1a92,
    0x8c9f8224b4bd740e,
    0x57659f7b3941ed3a,
    0x2ceee11897669151,
    0xd38153472fa3238c,
    0x88c04be02a8f4cfa,
    0xaa90572617b89654,
    0x8338c2cb82bffa91,
    0x840f2a46d47c626b,
    0x830e697e5708409a,
    0xde34567dea4547eb,
    0x2b4043aa10de6421,
    0xdbcc9d5414c00b6f,
    0x7c9895e4e4ec68fa,
    0x0bf34dd3400971c9,
    0x5d508c7733691ca1,
    0x2b092bc4c643b8f0,
    0x3c4d04597083f377,
    0x7b1a79cf33dc76ca,
    0x2e97af200bdf51ff,
];

fn digest(output: &[[f32; 8]]) -> u64 {
    output
        .iter()
        .flatten()
        .flat_map(|sample| sample.to_bits().to_le_bytes())
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
}

#[test]
fn both_rust_nodes_match_persistent_cpp_oracle_digests() {
    let mut seed = 0x7abeca01_u32;
    let mut reference_hashes = EXPECTED.into_iter();
    for step in [1, 5, 9] {
        for unsolid in 0..=4 {
            for unreflect in 0..=4 {
                let count = match (step, unsolid, unreflect) {
                    (1, 0, 0) => 480000,
                    _ => 20000,
                };
                let input: Vec<_> = (0..count)
                    .map(|sample| {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        match (sample, unsolid == unreflect) {
                            (0, _) => 1.0,
                            (1..=9999, false) => (seed as i32 as f32) * (0.01 / i32::MAX as f32),
                            _ => 0.0,
                        }
                    })
                    .collect();
                let mut first = vec![[0.0; 8]; count];
                let mut second = vec![[0.0; 8]; count];
                let mut original = OriginalNode::default();
                let mut borrowed = BorrowedNode::default();
                let parameters = NodeParameters::new(step, unsolid, unreflect).unwrap();
                input
                    .chunks(64)
                    .zip(first.chunks_mut(64))
                    .zip(second.chunks_mut(64))
                    .for_each(|((input, first), second)| {
                        original.process(input, first, parameters);
                        borrowed.process(input, second, parameters);
                    });
                let expected = reference_hashes.next().unwrap();
                assert_eq!(
                    digest(&first),
                    expected,
                    "baseline: damping {step}, UnSolid {unsolid}, UnReflect {unreflect}"
                );
                assert_eq!(
                    digest(&second),
                    expected,
                    "borrowed: damping {step}, UnSolid {unsolid}, UnReflect {unreflect}"
                );
            }
        }
    }
    assert!(reference_hashes.next().is_none());
}
