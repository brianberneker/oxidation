use oxidation_matrixverb_dsp::{
    dsp::core::{DspCore, Processor, Tuning},
    parameters::ProcessingParameters,
};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

const BLOCK: usize = 256;
const ITERATIONS: usize = (1 << 17) / BLOCK;

fn measure(label: &str, mut operation: impl FnMut()) {
    (0..ITERATIONS).for_each(|_| operation());
    let mut times = [0.0; 11];
    times.iter_mut().for_each(|time| {
        let start = Instant::now();
        (0..ITERATIONS).for_each(|_| operation());
        *time = start.elapsed().as_secs_f64() * 1e9 / (ITERATIONS * BLOCK) as f64;
    });
    times.sort_by(f64::total_cmp);
    println!(
        "{label}: median {:.3} ns/stereo frame, min {:.3}, max {:.3}",
        times[5], times[0], times[10]
    );
}

fn benchmark<const APPROXIMATE: bool, const ACCELERATED: bool>(input: &[[f32; 2]]) {
    let mut core = Processor::default();
    let tuning = Tuning::new(ProcessingParameters::default(), 44100.0).unwrap();
    let mut output = [[0.0; 2]; BLOCK];
    let label = format!(
        "{} math / acceleration {}",
        if APPROXIMATE {
            "Approximate"
        } else {
            "Original"
        },
        if ACCELERATED { "on" } else { "off" },
    );
    measure(&label, || {
        core.process::<APPROXIMATE, ACCELERATED>(
            black_box(input),
            black_box(&mut output),
            black_box(&tuning),
        );
        black_box(&output);
    });
}

fn profile<const APPROXIMATE: bool, const ACCELERATED: bool>() {
    let mut processor = Processor::default();
    let tuning = Tuning::new(ProcessingParameters::default(), 44100.0).unwrap();
    let input: [[f32; 2]; BLOCK] = std::array::from_fn(|frame| {
        [
            ((frame * 17 % 97) as f32 - 48.0) * 0.0001,
            ((frame * 29 % 97) as f32 - 48.0) * 0.0001,
        ]
    });
    let mut output = [[0.0; 2]; BLOCK];
    println!(
        "profiling PID {}, approximate={APPROXIMATE}, accelerated={ACCELERATED}",
        std::process::id()
    );
    let start = Instant::now();
    let mut frames = 0;
    while start.elapsed() < Duration::from_secs(12) {
        (0..256).for_each(|_| {
            processor.process::<APPROXIMATE, ACCELERATED>(
                black_box(&input),
                black_box(&mut output),
                black_box(&tuning),
            );
            black_box(&output);
        });
        frames += 256 * BLOCK;
    }
    println!(
        "{:.3} ns/frame",
        start.elapsed().as_secs_f64() * 1e9 / frames as f64
    );
}

fn benchmark_integrated(input: &[[f32; 2]], original: bool, accelerated: bool) {
    let mut core = DspCore::default();
    core.init(44100.0, original).unwrap();
    let mut output = [[0.0; 2]; BLOCK];
    let label = format!(
        "Integrated dual-backend plugin DSP / {} math / acceleration {}",
        if original { "Original" } else { "Approximate" },
        if accelerated { "on" } else { "off" },
    );
    measure(&label, || {
        core.process(
            black_box(input),
            black_box(&mut output),
            ProcessingParameters::default(),
            original,
            accelerated,
        )
        .unwrap();
        black_box(&output);
    });
}

fn main() -> Result<(), &'static str> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if let [command, mode] = arguments.as_slice() {
        if command == "profile" {
            match mode.as_str() {
                "os" => profile::<false, false>(),
                "as" => profile::<true, false>(),
                "oh" => profile::<false, true>(),
                "ah" => profile::<true, true>(),
                _ => return Err("profile mode must be os, as, oh, or ah"),
            }
            return Ok(());
        }
    }
    let reverse = match arguments.as_slice() {
        [] => false,
        [command] if command == "reverse" => true,
        _ => return Err("usage: matrixverb-bench [reverse | profile os|as|oh|ah]"),
    };
    let input: Vec<_> = (1..=BLOCK)
        .map(|frame| {
            [
                ((frame * 17 % 97) as f32 - 48.0) * 0.0001,
                ((frame * 29 % 97) as f32 - 48.0) * 0.0001,
            ]
        })
        .collect();
    println!("Whole MatrixVerb processor, 44.1 kHz, defaults, block={BLOCK}, 11 rounds");
    println!("Acceleration: {}", oxidation_dsp::acceleration::backend());
    if reverse {
        benchmark::<true, true>(&input);
        benchmark::<false, true>(&input);
        benchmark::<true, false>(&input);
        benchmark::<false, false>(&input);
    } else {
        benchmark::<false, false>(&input);
        benchmark::<true, false>(&input);
        benchmark::<false, true>(&input);
        benchmark::<true, true>(&input);
    }
    let mut modes = [(true, false), (false, false), (true, true), (false, true)];
    if reverse {
        modes.reverse();
    }
    for (original, accelerated) in modes {
        benchmark_integrated(&input, original, accelerated);
    }
    Ok(())
}
