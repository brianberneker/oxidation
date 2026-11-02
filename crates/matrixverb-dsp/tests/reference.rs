use oxidation_matrixverb_dsp::{
    dsp::core::{Processor, Tuning},
    parameters::ProcessingParameters,
};
use std::process::Command;

const FRAMES: usize = 32768;

fn parameters(scenario: usize, position: usize) -> ProcessingParameters {
    let cases = [
        [1.0, 0.0, 0.0, 0.0, 0.5, 0.5, 1.0],
        [0.0; 7],
        [1.0; 7],
        [0.2, 0.6, 0.9, 1.0, 0.0, 0.5, 0.4],
        [0.83, 0.13, 0.71, 0.53, 0.37, 0.13, 0.71],
        [0.83, 0.13, 0.71, 0.53, 0.37, 0.91, 0.71],
        [1.0, 1.0, 0.2, 0.5, 0.2, 0.5, 1.0],
        [0.1, 0.5, 0.5, 0.5, 0.1, 0.5, 0.0],
    ];
    let index = match (scenario, position) {
        (9, 0..8192) => 2,
        (9, 8192..16384) => 3,
        (9, _) => 4,
        (0..=7, _) => scenario,
        _ => 0,
    };
    let [filter, damping, speed, vibrato, room_size, flavor, wet] = cases[index];
    ProcessingParameters {
        filter,
        damping,
        speed,
        vibrato,
        room_size,
        flavor,
        wet,
    }
}

fn input(frame: usize, channel: usize, scenario: usize) -> f32 {
    match (scenario, frame) {
        (8, _) | (_, 10000..) => 0.0,
        (_, 0) => {
            if channel == 0 {
                1.0
            } else {
                -0.5
            }
        }
        _ => (((frame * if channel == 0 { 17 } else { 29 }) % 97) as f32 - 48.0) * 0.0001,
    }
}

#[test]
#[ignore = "requires pinned C++ reference; run sh scripts/matrixverb-reference.sh"]
fn all_four_whole_processor_modes_match_seeded_upstream_within_their_error_budgets() {
    let runner = std::env::var("MATRIXVERB_REFERENCE")
        .expect("set MATRIXVERB_REFERENCE to the compiled upstream runner");
    let mut peaks32 = [0.0_f64; 4];
    let mut peaks64 = [0.0_f64; 4];
    for rate in [22050.0, 44100.0, 48000.0, 96000.0, 192000.0, 384000.0] {
        for scenario in 0..10 {
            let input32: Vec<[f32; 2]> = (0..FRAMES)
                .map(|frame| std::array::from_fn(|channel| input(frame, channel, scenario)))
                .collect();
            let input64: Vec<_> = input32.iter().map(|frame| frame.map(f64::from)).collect();
            for precision in ["f32", "f64"] {
                let result = Command::new(&runner)
                    .args([rate.to_string(), scenario.to_string(), precision.to_owned()])
                    .output()
                    .unwrap();
                assert!(
                    result.status.success(),
                    "C++ failed: {}",
                    String::from_utf8_lossy(&result.stderr)
                );
                let width = if precision == "f32" { 4 } else { 8 };
                assert_eq!(
                    result.stdout.len(),
                    FRAMES * 2 * width,
                    "truncated reference output"
                );
                let reference: Vec<f64> = result
                    .stdout
                    .chunks_exact(width)
                    .map(|bytes| {
                        if precision == "f32" {
                            f64::from(f32::from_le_bytes(bytes.try_into().unwrap()))
                        } else {
                            f64::from_le_bytes(bytes.try_into().unwrap())
                        }
                    })
                    .collect();
                for mode in 0..4 {
                    let actual = match mode {
                        0 => render::<false, false>(&input32, &input64, scenario, rate, precision),
                        1 => render::<true, false>(&input32, &input64, scenario, rate, precision),
                        2 => render::<false, true>(&input32, &input64, scenario, rate, precision),
                        _ => render::<true, true>(&input32, &input64, scenario, rate, precision),
                    };
                    let mut peak = 0.0_f64;
                    let mut squared = 0.0;
                    actual.iter().zip(&reference).enumerate().for_each(
                        |(frame, (actual, expected))| {
                            assert!(
                                actual.is_finite() && expected.is_finite(),
                                "nonfinite sample: {rate}/{scenario}/{precision}/{frame}"
                            );
                            let error = (actual - expected).abs();
                            peak = peak.max(error);
                            squared += error * error;
                        },
                    );
                    let rms = (squared / actual.len() as f64).sqrt();
                    if precision == "f32" {
                        peaks32[mode] = peaks32[mode].max(peak);
                        assert!(
                            peak <= 2e-6 && rms <= 1e-7,
                            "f32: mode {mode}, {rate}/{scenario}, peak {peak}, RMS {rms}"
                        );
                    } else {
                        peaks64[mode] = peaks64[mode].max(peak);
                        let (peak_budget, rms_budget) = if mode == 0 {
                            (1e-10, 1e-12)
                        } else {
                            (1e-5, 1e-6)
                        };
                        assert!(
                            peak <= peak_budget && rms <= rms_budget,
                            "f64: mode {mode}, {rate}/{scenario}, peak {peak}, RMS {rms}"
                        );
                    }
                }
            }
        }
    }
    println!("Whole MatrixVerb comparison, modes O/S, A/S, O/H, A/H:");
    println!("Peak f32 errors: {peaks32:?}");
    println!("Peak f64 errors: {peaks64:?}");
}

fn render<const APPROXIMATE: bool, const ACCELERATED: bool>(
    input32: &[[f32; 2]],
    input64: &[[f64; 2]],
    scenario: usize,
    rate: f64,
    precision: &str,
) -> Vec<f64> {
    let mut processor = Processor::default();
    let mut output32 = vec![[0.0_f32; 2]; FRAMES];
    let mut output64 = vec![[0.0_f64; 2]; FRAMES];
    for position in (0..FRAMES).step_by(256) {
        let tuning = Tuning::new(parameters(scenario, position), rate).unwrap();
        let range = position..position + 256;
        if precision == "f32" {
            processor.process::<APPROXIMATE, ACCELERATED>(
                &input32[range.clone()],
                &mut output32[range],
                &tuning,
            );
        } else {
            processor.process_f64::<APPROXIMATE, ACCELERATED>(
                &input64[range.clone()],
                &mut output64[range],
                &tuning,
            );
        }
    }
    if precision == "f32" {
        output32.iter().flatten().copied().map(f64::from).collect()
    } else {
        output64.iter().flatten().copied().collect()
    }
}
