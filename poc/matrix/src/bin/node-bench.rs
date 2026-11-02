use oxidation_dsp::{
    dsp::core::{ApproximateNode, BorrowedNode, OriginalNode},
    parameters::NodeParameters,
};
use std::{
    env,
    error::Error,
    fs,
    hint::black_box,
    time::{Duration, Instant},
};

const ROUNDS: usize = 11;
const SAMPLES_PER_ROUND: usize = 1 << 19;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    match arguments.as_slice() {
        [] => benchmark(false),
        [command] if command == "reverse" => benchmark(true),
        [command, path] if command == "verify" => verify(path)?,
        [command, mode] if command == "profile" => profile(mode)?,
        _ => {
            return Err(
                "usage: node-bench [reverse | verify REFERENCE.bin | profile os|as|oh|ah]".into(),
            )
        }
    }
    Ok(())
}

fn profile(mode: &str) -> Result<(), Box<dyn Error>> {
    let (original_math, accelerated) = match mode {
        "os" => (true, false),
        "as" => (false, false),
        "oh" => (true, true),
        "ah" => (false, true),
        _ => return Err("profile mode must be os, as, oh, or ah".into()),
    };
    let input: [f32; 256] = std::array::from_fn(|frame| ((frame * 17 % 97) as f32 - 48.0) * 0.0001);
    let mut output = [[0.0; 8]; 256];
    let mut original = BorrowedNode::default();
    let mut approximate = ApproximateNode::default();
    let parameters = NodeParameters::new(5, 0, 0).unwrap();
    println!("profiling PID {}, mode {mode}", std::process::id());
    let start = Instant::now();
    let mut frames = 0;
    while start.elapsed() < Duration::from_secs(12) {
        (0..256).for_each(|_| {
            if original_math {
                original.process_with_acceleration(
                    black_box(&input),
                    black_box(&mut output),
                    black_box(parameters),
                    accelerated,
                );
            } else {
                approximate.process_with_acceleration(
                    black_box(&input),
                    black_box(&mut output),
                    black_box(parameters),
                    accelerated,
                );
            }
            black_box(&output);
        });
        frames += 256 * input.len();
    }
    println!(
        "{:.3} ns/sample",
        start.elapsed().as_secs_f64() * 1e9 / frames as f64
    );
    Ok(())
}

fn benchmark(reverse: bool) {
    let block = 256;
    let input: Vec<_> = (0..block)
        .map(|frame| ((frame * 17 % 97) as f32 - 48.0) * 0.0001)
        .collect();
    let mut output = vec![[0.0; 8]; block];
    let parameters = NodeParameters::new(5, 0, 0).unwrap();
    let mut original = BorrowedNode::default();
    let mut approximate = ApproximateNode::default();
    println!("Ultralight3 core only: block={block}, damping=5, UnSolid=0, UnReflect=0");
    println!("Acceleration: {}", oxidation_dsp::acceleration::backend());

    let modes = [
        ("Original math / acceleration off", true, false),
        ("Approximate math / acceleration off", false, false),
        ("Original math / acceleration on", true, true),
        ("Approximate math / acceleration on", false, true),
    ];
    let order = if reverse { [3, 2, 1, 0] } else { [0, 1, 2, 3] };
    for mode in order {
        let (name, original_math, accelerated) = modes[mode];
        original.reset();
        approximate.reset();
        let run = || {
            if original_math {
                original.process_with_acceleration(
                    black_box(&input),
                    black_box(&mut output),
                    black_box(parameters),
                    accelerated,
                );
            } else {
                approximate.process_with_acceleration(
                    black_box(&input),
                    black_box(&mut output),
                    black_box(parameters),
                    accelerated,
                );
            }
            black_box(&output);
        };
        measure(name, block, run);
    }
}

fn measure(label: &str, block: usize, mut operation: impl FnMut()) {
    let iterations = SAMPLES_PER_ROUND / block;
    (0..iterations).for_each(|_| operation());
    let mut samples: Vec<_> = (0..ROUNDS)
        .map(|_| {
            let start = Instant::now();
            (0..iterations).for_each(|_| operation());
            start.elapsed().as_secs_f64() * 1e9 / (block * iterations) as f64
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    println!(
        "{label}: median {:.3} ns/sample, min {:.3}, max {:.3}",
        samples[ROUNDS / 2],
        samples[0],
        samples[ROUNDS - 1]
    );
}

struct Records<'a>(&'a [u8]);

impl Records<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], Box<dyn Error>> {
        let result = self
            .0
            .get(..N)
            .ok_or("truncated node reference")?
            .try_into()?;
        self.0 = &self.0[N..];
        Ok(result)
    }

    fn f32(&mut self) -> Result<f32, Box<dyn Error>> {
        Ok(f32::from_le_bytes(self.take()?))
    }
}

fn verify(path: &str) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let comparisons = verify_records(&bytes)?;
    println!("PASS: {comparisons} bit-identical node outputs per Rust implementation");
    Ok(())
}

fn verify_records(bytes: &[u8]) -> Result<usize, Box<dyn Error>> {
    let mut records = Records(bytes);
    if records.take::<8>()? != *b"AWNODE01" {
        return Err("incorrect node reference magic".into());
    }
    let cases = u32::from_le_bytes(records.take()?);
    if cases == 0 {
        return Err("empty node corpus".into());
    }
    let mut comparisons = 0;
    for case in 0..cases {
        let [step, unsolid, unreflect, reserved] = records.take()?;
        if reserved != 0 {
            return Err("nonzero reserved node header byte".into());
        }
        let parameters = NodeParameters::new(step, unsolid, unreflect)?;
        let count = usize::try_from(u32::from_le_bytes(records.take()?))?;
        if count == 0 || count > records.0.len() / 36 {
            return Err("invalid or truncated node record count".into());
        }
        let mut input = Vec::with_capacity(count);
        let mut expected = Vec::with_capacity(count);
        for _ in 0..count {
            input.push(records.f32()?);
            expected.push([
                records.f32()?,
                records.f32()?,
                records.f32()?,
                records.f32()?,
                records.f32()?,
                records.f32()?,
                records.f32()?,
                records.f32()?,
            ]);
        }
        for block in [1, 16, 47, 64, 256, 1024] {
            let mut original = OriginalNode::default();
            let mut borrowed = BorrowedNode::default();
            let mut first = vec![[0.0; 8]; count];
            let mut second = vec![[0.0; 8]; count];
            input
                .chunks(block)
                .zip(first.chunks_mut(block))
                .zip(second.chunks_mut(block))
                .for_each(|((input, first), second)| {
                    original.process(input, first, parameters);
                    borrowed.process(input, second, parameters);
                });
            for (sample, ((expected, first), second)) in
                expected.iter().zip(&first).zip(&second).enumerate()
            {
                if expected.map(f32::to_bits) != first.map(f32::to_bits) {
                    return Err(format!(
                        "baseline mismatch: case {case}, block {block}, sample {sample}"
                    )
                    .into());
                }
                if expected.map(f32::to_bits) != second.map(f32::to_bits) {
                    return Err(format!(
                        "borrowed-window mismatch: case {case}, block {block}, sample {sample}"
                    )
                    .into());
                }
            }
            comparisons += count * 8;
        }
    }
    if !records.0.is_empty() {
        return Err("unexpected trailing node reference data".into());
    }
    Ok(comparisons)
}

#[cfg(test)]
mod tests {
    use super::verify_records;

    fn silence() -> Vec<u8> {
        let mut bytes = b"AWNODE01".to_vec();
        bytes.extend(1_u32.to_le_bytes());
        bytes.extend([5, 4, 4, 0]);
        bytes.extend(1_u32.to_le_bytes());
        bytes.extend([0; 36]);
        bytes
    }

    #[test]
    fn accepts_silence() {
        assert_eq!(verify_records(&silence()).unwrap(), 48);
    }

    #[test]
    fn rejects_all_truncations_and_trailing_data() {
        let mut bytes = silence();
        (0..bytes.len()).for_each(|length| {
            assert!(verify_records(&bytes[..length]).is_err());
        });
        bytes.push(0);
        assert!(verify_records(&bytes).is_err());
    }

    #[test]
    fn rejects_invalid_headers_and_mismatches() {
        [0, 8, 12, 15, 16, 24].into_iter().for_each(|offset| {
            let mut bytes = silence();
            bytes[offset] = 255;
            assert!(verify_records(&bytes).is_err());
        });
    }
}
