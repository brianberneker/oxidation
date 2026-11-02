use oxidation_matrixverb_dsp::{
    dsp::core::{DspCore, Processor, Tuning},
    parameters::ProcessingParameters,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct CountingAllocator;
thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn allocated() {
    COUNTING.with(|enabled| {
        if enabled.get() {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        allocated();
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn input(count: usize) -> Vec<[f32; 2]> {
    (0..count)
        .map(|frame| {
            if frame == 0 {
                [1.0, -0.5]
            } else {
                [
                    ((frame * 17 % 97) as f32 - 48.0) * 0.0001,
                    ((frame * 29 % 97) as f32 - 48.0) * 0.0001,
                ]
            }
        })
        .collect()
}

fn render(
    input: &[[f32; 2]],
    block: usize,
    original: bool,
    accelerated: bool,
    rate: f64,
) -> Vec<[f32; 2]> {
    let mut core = DspCore::default();
    core.init(rate, original).unwrap();
    let mut output = vec![[0.0; 2]; input.len()];
    input
        .chunks(block)
        .zip(output.chunks_mut(block))
        .for_each(|(input, output)| {
            core.process(
                input,
                output,
                ProcessingParameters::default(),
                original,
                accelerated,
            )
            .unwrap();
        });
    output
}

#[test]
fn all_modes_are_block_invariant_and_finite_at_supported_rates() {
    let input = input(20000);
    for rate in [22050.0, 44100.0, 48000.0, 96000.0, 192000.0, 384000.0] {
        for (original, accelerated) in [(true, false), (false, false), (true, true), (false, true)]
        {
            let expected = render(&input, 256, original, accelerated, rate);
            assert!(expected.iter().flatten().all(|sample| sample.is_finite()));
            for block in [1, 17, 1024] {
                assert_eq!(
                    render(&input, block, original, accelerated, rate),
                    expected,
                    "{rate}/{original}/{accelerated}/{block}"
                );
            }
        }
    }
}

#[test]
fn all_four_integrated_modes_match_their_specialized_processor_including_dither() {
    let input = input(32768);
    let tuning = Tuning::new(ProcessingParameters::default(), 44100.0).unwrap();
    for (original, accelerated) in [(true, false), (false, false), (true, true), (false, true)] {
        let expected = render(&input, 64, original, accelerated, 44100.0);
        let mut output = vec![[0.0; 2]; input.len()];
        let mut processor = Processor::default();
        match (original, accelerated) {
            (true, false) => processor.process::<false, false>(&input, &mut output, &tuning),
            (false, false) => processor.process::<true, false>(&input, &mut output, &tuning),
            (true, true) => processor.process::<false, true>(&input, &mut output, &tuning),
            (false, true) => processor.process::<true, true>(&input, &mut output, &tuning),
        }
        assert_eq!(
            output, expected,
            "original={original}, accelerated={accelerated}"
        );
    }
}

#[test]
fn all_controls_affect_the_signal_and_modes_are_distinct() {
    let input = input(48000);
    let tuning = Tuning::new(ProcessingParameters::default(), 44100.0).unwrap();
    let mut baseline = vec![[0.0; 2]; input.len()];
    Processor::default().process_f64::<false, false>(
        &input
            .iter()
            .map(|frame| frame.map(f64::from))
            .collect::<Vec<_>>(),
        &mut baseline,
        &tuning,
    );
    for control in 0..7 {
        let mut p = ProcessingParameters::default();
        match control {
            0 => p.filter = 0.0,
            1 => p.damping = 1.0,
            2 => p.speed = 1.0,
            3 => p.vibrato = 1.0,
            4 => p.room_size = 1.0,
            5 => p.flavor = 0.0,
            _ => p.wet = 0.0,
        }
        let mut actual = vec![[0.0; 2]; input.len()];
        Processor::default().process_f64::<false, false>(
            &input
                .iter()
                .map(|frame| frame.map(f64::from))
                .collect::<Vec<_>>(),
            &mut actual,
            &Tuning::new(p, 44100.0).unwrap(),
        );
        assert!(
            actual
                .iter()
                .flatten()
                .zip(baseline.iter().flatten())
                .any(|(a, b)| (a - b).abs() > 1e-7),
            "control {control}"
        );
    }
    let input64: Vec<_> = input.iter().map(|frame| frame.map(f64::from)).collect();
    let mut approximate = vec![[0.0; 2]; input.len()];
    let mut accelerated = approximate.clone();
    let mut both = approximate.clone();
    Processor::default().process_f64::<true, false>(&input64, &mut approximate, &tuning);
    Processor::default().process_f64::<false, true>(&input64, &mut accelerated, &tuning);
    Processor::default().process_f64::<true, true>(&input64, &mut both, &tuning);
    for output in [&approximate, &accelerated, &both] {
        assert_ne!(output, &baseline);
    }
    assert_ne!(approximate, both);
    assert_ne!(accelerated, both);
    assert_ne!(approximate, accelerated);
}

#[test]
fn ten_second_tails_and_approximation_error_are_bounded() {
    let mut impulse = vec![[0.0; 2]; 441000];
    impulse[0] = [1.0, -0.5];
    for flavor in [0.0, 0.5, 1.0] {
        let p = ProcessingParameters {
            flavor,
            vibrato: 1.0,
            speed: 1.0,
            damping: 0.0,
            ..ProcessingParameters::default()
        };
        let tuning = Tuning::new(p, 44100.0).unwrap();
        let mut original = vec![[0.0; 2]; impulse.len()];
        let mut approximate = original.clone();
        let mut accelerated = original.clone();
        let mut both = original.clone();
        Processor::default().process_f64::<false, false>(&impulse, &mut original, &tuning);
        Processor::default().process_f64::<true, false>(&impulse, &mut approximate, &tuning);
        Processor::default().process_f64::<false, true>(&impulse, &mut accelerated, &tuning);
        Processor::default().process_f64::<true, true>(&impulse, &mut both, &tuning);
        for output in [&approximate, &accelerated, &both] {
            let mut peak = 0.0_f64;
            let mut error = 0.0_f64;
            output
                .iter()
                .flatten()
                .zip(original.iter().flatten())
                .for_each(|(a, b)| {
                    assert!(a.is_finite() && a.abs() < 4.0);
                    peak = peak.max((a - b).abs());
                    error += (a - b) * (a - b);
                });
            let rms = (error / (impulse.len() * 2) as f64).sqrt();
            assert!(
                peak < 1e-5 && rms < 1e-6,
                "flavor {flavor}: peak {peak}, RMS {rms}"
            );
        }
    }
}

#[test]
fn processing_switches_and_reset_do_not_allocate() {
    let mut core = DspCore::default();
    core.init(48000.0, true).unwrap();
    let input = [[0.001, -0.002]; 64];
    let mut output = [[0.0; 2]; 64];
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|enabled| enabled.set(true));
    for block in 0..2000 {
        core.process(
            &input,
            &mut output,
            ProcessingParameters {
                room_size: (block % 100) as f32 / 100.0,
                ..ProcessingParameters::default()
            },
            block % 2 == 0,
            block % 3 == 0,
        )
        .unwrap();
    }
    core.reset(false);
    COUNTING.with(|enabled| enabled.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}

#[test]
fn rejects_invalid_controls_and_rates_without_mutating_state() {
    let mut core = DspCore::default();
    for rate in [f64::NAN, f64::INFINITY, 0.0, 8000.0, 400000.0] {
        assert!(core.init(rate, true).is_err());
    }
    for value in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert!(core
            .process(
                &[[1.0; 2]],
                &mut [[0.0; 2]],
                ProcessingParameters {
                    filter: value,
                    ..ProcessingParameters::default()
                },
                true,
                false
            )
            .is_err());
    }
    let mut actual = [[0.0; 2]; 64];
    let input = [[0.001; 2]; 64];
    core.process(
        &input,
        &mut actual,
        ProcessingParameters::default(),
        true,
        false,
    )
    .unwrap();
    let mut expected = actual;
    DspCore::default()
        .process(
            &input,
            &mut expected,
            ProcessingParameters::default(),
            true,
            false,
        )
        .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn reset_restores_seeded_initial_state_for_each_mode() {
    let input = input(16384);
    for (original, accelerated) in [(true, false), (false, false), (true, true), (false, true)] {
        let mut core = DspCore::default();
        core.init(48000.0, original).unwrap();
        let mut first = vec![[0.0; 2]; input.len()];
        let mut second = first.clone();
        core.process(
            &input,
            &mut first,
            ProcessingParameters::default(),
            original,
            accelerated,
        )
        .unwrap();
        core.reset(original);
        core.process(
            &input,
            &mut second,
            ProcessingParameters::default(),
            original,
            accelerated,
        )
        .unwrap();
        assert_eq!(first, second);
    }
}

#[test]
#[should_panic(expected = "assertion `left == right` failed")]
fn mismatched_buffers_are_rejected() {
    DspCore::default()
        .process(
            &[[0.0; 2]],
            &mut [],
            ProcessingParameters::default(),
            true,
            false,
        )
        .unwrap();
}
