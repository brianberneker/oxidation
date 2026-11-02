use oxidation_dsp::{dsp::core::DspCore, parameters::Parameters};
use std::{
    env,
    error::Error,
    fs,
    hint::black_box,
    time::{Duration, Instant},
};

const FRAMES: usize = 4096;
const ROUNDS: usize = 21;
const ITERATIONS: usize = 4096;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    match arguments.as_slice() {
        [] => benchmark(),
        [command, path] if command == "verify" => verify(path)?,
        _ => return Err("usage: oxidation-dsp [verify REFERENCE.bin]".into()),
    }
    Ok(())
}

fn benchmark() {
    let input64: Vec<_> = (0..FRAMES)
        .map(|frame| {
            let x = (frame as f64 + 1.0) / FRAMES as f64;
            [x, -x * 0.37, x * 0.13, -x * 0.81]
        })
        .collect();
    let input32: Vec<_> = input64
        .iter()
        .map(|frame| frame.map(|x| x as f32))
        .collect();
    let mut output64 = vec![[0.0; 4]; FRAMES];
    let mut output32 = vec![[0.0; 4]; FRAMES];
    let parameters = Parameters { regeneration: 0.47 };
    println!("kernel-only, {FRAMES} frames/block, {ROUNDS} rounds, {ITERATIONS} calls/round");
    measure("MatrixVerb f64", || {
        DspCore::matrixverb(
            black_box(&input64),
            black_box(&mut output64),
            black_box(parameters),
        );
        black_box(&output64);
    });
    measure("Ultralight3 f32", || {
        DspCore::ultralight3(black_box(&input32), black_box(&mut output32));
        black_box(&output32);
    });
}

fn measure(name: &str, mut operation: impl FnMut()) {
    (0..ITERATIONS).for_each(|_| operation());
    let mut samples: Vec<_> = (0..ROUNDS)
        .map(|_| {
            let start = Instant::now();
            (0..ITERATIONS).for_each(|_| operation());
            start.elapsed()
        })
        .collect();
    samples.sort_unstable();
    let ns_per_frame =
        |duration: Duration| duration.as_secs_f64() * 1e9 / (FRAMES * ITERATIONS) as f64;
    println!(
        "{name}: median {:.3} ns/transform, min {:.3}, max {:.3}",
        ns_per_frame(samples[ROUNDS / 2]),
        ns_per_frame(samples[0]),
        ns_per_frame(samples[ROUNDS - 1]),
    );
}

struct Records<'a> {
    bytes: &'a [u8],
}

impl Records<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], Box<dyn Error>> {
        let bytes = self.bytes.get(..N).ok_or("truncated reference file")?;
        let result = bytes.try_into()?;
        self.bytes = &self.bytes[N..];
        Ok(result)
    }

    fn f64(&mut self) -> Result<f64, Box<dyn Error>> {
        Ok(f64::from_le_bytes(self.take()?))
    }

    fn f32(&mut self) -> Result<f32, Box<dyn Error>> {
        Ok(f32::from_le_bytes(self.take()?))
    }
}

fn verify(path: &str) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let count = verify_records(&bytes)?;
    println!(
        "PASS: {count} reference records, {} bit-identical outputs",
        count * 8
    );
    Ok(())
}

fn verify_records(bytes: &[u8]) -> Result<usize, Box<dyn Error>> {
    let mut records = Records { bytes };
    if records.take::<8>()? != *b"AWMATR01" {
        return Err("incorrect reference file magic".into());
    }
    let count = usize::try_from(u64::from_le_bytes(records.take()?))?;
    if count == 0 {
        return Err("reference file contains no comparisons".into());
    }
    for record in 0..count {
        let input = [
            records.f64()?,
            records.f64()?,
            records.f64()?,
            records.f64()?,
        ];
        let parameters = Parameters {
            regeneration: records.f64()?,
        };
        let expected = [
            records.f64()?,
            records.f64()?,
            records.f64()?,
            records.f64()?,
        ];
        let mut output = [[0.0; 4]];
        DspCore::matrixverb(&[input], &mut output, parameters);
        let [actual] = output;
        if actual.map(f64::to_bits) != expected.map(f64::to_bits) {
            return Err(format!("MatrixVerb bit mismatch at record {record}: {input:?}").into());
        }
        let input = [
            records.f32()?,
            records.f32()?,
            records.f32()?,
            records.f32()?,
        ];
        let expected = [
            records.f32()?,
            records.f32()?,
            records.f32()?,
            records.f32()?,
        ];
        let mut output = [[0.0; 4]];
        DspCore::ultralight3(&[input], &mut output);
        let [actual] = output;
        if actual.map(f32::to_bits) != expected.map(f32::to_bits) {
            return Err(format!("Ultralight3 bit mismatch at record {record}: {input:?}").into());
        }
    }
    if !records.bytes.is_empty() {
        return Err("unexpected trailing data in reference file".into());
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::verify_records;

    fn valid_record() -> Vec<u8> {
        let mut bytes = b"AWMATR01".to_vec();
        bytes.extend(1_u64.to_le_bytes());
        [1.0_f64, 2.0, 4.0, 8.0, 0.5, -6.5, -5.5, -3.5, 0.5]
            .iter()
            .for_each(|value| bytes.extend(value.to_le_bytes()));
        [1.0_f32, 2.0, 4.0, 8.0, -13.0, -11.0, -7.0, 1.0]
            .iter()
            .for_each(|value| bytes.extend(value.to_le_bytes()));
        bytes
    }

    #[test]
    fn accepts_complete_record() {
        assert_eq!(verify_records(&valid_record()).unwrap(), 1);
    }

    #[test]
    fn rejects_every_truncation() {
        let bytes = valid_record();
        (0..bytes.len()).for_each(|length| {
            assert!(verify_records(&bytes[..length]).is_err());
        });
    }

    #[test]
    fn rejects_bad_magic_empty_corpus_and_trailing_data() {
        let mut bytes = valid_record();
        bytes[0] = b'X';
        assert!(verify_records(&bytes).is_err());
        let mut bytes = b"AWMATR01".to_vec();
        bytes.extend(0_u64.to_le_bytes());
        assert!(verify_records(&bytes).is_err());
        let mut bytes = valid_record();
        bytes.push(0);
        assert!(verify_records(&bytes).is_err());
    }

    #[test]
    fn rejects_bit_mismatch_in_each_kernel() {
        [56, 104].into_iter().for_each(|offset| {
            let mut bytes = valid_record();
            bytes[offset] ^= 1;
            assert!(verify_records(&bytes).is_err());
        });
    }
}
