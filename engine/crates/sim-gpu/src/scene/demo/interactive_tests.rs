//! Interactive paint regressions for the production-sized resting scene.

use super::*;
use crate::scene::{GpuPaintMaterial, coupled::GpuTickOutcome};
use particle_sim::OUTER_DT_S;
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

const TARGET_TICKS: u64 = 300;

struct Notify(std::thread::Thread);

impl Wake for Notify {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker: Waker = Arc::new(Notify(std::thread::current())).into();
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => {
                assert!(
                    Instant::now() < deadline,
                    "interactive GPU regression timed out"
                );
                std::thread::park_timeout(Duration::from_millis(10));
            }
        }
    }
}

#[test]
#[ignore = "requires a native Metal GPU and advances the 480x270 interactive scene"]
fn native_metal_default_water_brush_in_air_accepts_300_ticks() {
    run_paint_case(
        "water-in-air-radius4",
        [240, 100],
        4,
        GpuPaintMaterial::Water,
    );
}

#[test]
#[ignore = "requires a native Metal GPU and advances the 480x270 interactive scene"]
fn native_metal_default_water_brush_at_surface_accepts_300_ticks() {
    run_paint_case(
        "water-at-surface-radius4",
        [240, 241],
        4,
        GpuPaintMaterial::Water,
    );
}

#[test]
#[ignore = "requires a native Metal GPU and advances the 480x270 interactive scene"]
fn native_metal_default_air_brush_in_water_accepts_300_ticks() {
    run_paint_case("air-in-water-radius4", [240, 248], 4, GpuPaintMaterial::Air);
}

#[test]
#[ignore = "requires a native Metal GPU and advances the 480x270 interactive scene"]
fn native_metal_maximum_water_brush_in_air_accepts_300_ticks() {
    run_paint_case(
        "water-in-air-radius16",
        [240, 100],
        16,
        GpuPaintMaterial::Water,
    );
}

fn run_paint_case(label: &str, center: [u32; 2], radius: u32, material: GpuPaintMaterial) {
    block_on(async {
        let context = GpuContext::new().await.expect("native GPU device");
        assert_eq!(context.backend(), "Metal", "this regression requires Metal");
        let mut scene = m3_demo_scene(&context, 7).expect("resting production scene");
        let before_paint = scene.progress();
        let rest = scene
            .checkpoint_prototype()
            .await
            .expect("initial checkpoint");
        scene
            .paint(center[0], center[1], radius, material)
            .expect("bounded brush edit");
        let after_paint = scene.progress();
        assert_eq!(after_paint.tick, before_paint.tick);
        assert_eq!(after_paint.accepted_time_s, before_paint.accepted_time_s);
        assert_eq!(
            after_paint.remaining_outer_s,
            before_paint.remaining_outer_s
        );
        assert_eq!(after_paint.state_revision, before_paint.state_revision + 1);
        let painted = scene
            .checkpoint_prototype()
            .await
            .expect("painted checkpoint");
        let initial_fields = checkpoint_fields(&painted);
        assert_ne!(
            checkpoint_fields(&rest)[0],
            initial_fields[0],
            "brush must change committed phase masses"
        );
        let center_cell = center[1] as usize * WIDTH + center[0] as usize;
        let selected_phase = usize::from(material == GpuPaintMaterial::Air);
        assert!(initial_fields[0][selected_phase * WIDTH * HEIGHT + center_cell] > 0.0);
        assert_eq!(
            initial_fields[0][(1 - selected_phase) * WIDTH * HEIGHT + center_cell],
            0.0
        );
        let initial_inventory = inventory(&initial_fields);
        assert!(
            initial_fields[3]
                .iter()
                .chain(&initial_fields[4])
                .all(|&speed| speed == 0.0)
        );
        let started = Instant::now();
        let mut accepted_substeps = 0_u64;
        let mut max_speed = 0.0_f32;
        let mut max_changed_phase_slots = 0;
        let mut before_bytes = painted;
        for _ in 0..TARGET_TICKS * 128 {
            let before_progress = scene.progress();
            match scene.advance_outer_tick(1).await {
                Ok(outcome) => {
                    let (accepted, pressure) = match outcome {
                        GpuTickOutcome::Paused {
                            accepted_substeps, ..
                        } => (accepted_substeps, None),
                        GpuTickOutcome::Complete {
                            accepted_substeps,
                            last_pressure,
                            encoded_time_error_s,
                            ..
                        } => {
                            assert!(encoded_time_error_s.abs() < 1e-9);
                            (accepted_substeps, Some(last_pressure))
                        }
                    };
                    assert_eq!(accepted, 1);
                    accepted_substeps += u64::from(accepted);
                    let progress = scene.progress();
                    assert_eq!(progress.state_revision, before_progress.state_revision + 1);
                    let accepted_bytes = scene
                        .checkpoint_prototype()
                        .await
                        .expect("accepted checkpoint");
                    if let Some(pressure) = pressure {
                        let gate = PressureConfig::default();
                        assert!(pressure.scaled_residual <= gate.scaled_residual_tolerance);
                        assert!(pressure.scaled_divergence <= gate.scaled_divergence_tolerance);
                        assert_eq!(progress.remaining_outer_s, 0.0);
                        assert!(
                            (progress.accepted_time_s - progress.tick as f64 * OUTER_DT_S).abs()
                                < 1e-9
                        );
                        if [1, 25, 50, 100, 200, TARGET_TICKS].contains(&progress.tick) {
                            let fields = checkpoint_fields(&accepted_bytes);
                            assert_inventory_conserved(initial_inventory, inventory(&fields));
                            max_speed = max_speed.max(
                                fields[3]
                                    .iter()
                                    .chain(&fields[4])
                                    .map(|v| v.abs())
                                    .fold(0.0, f32::max),
                            );
                            let changed_cells = fields[0]
                                .iter()
                                .zip(&initial_fields[0])
                                .filter(|(actual, initial)| actual.to_bits() != initial.to_bits())
                                .count();
                            max_changed_phase_slots = max_changed_phase_slots.max(changed_cells);
                            eprintln!(
                                "INTERACTIVE {label}: tick={}, accepted_substeps={accepted_substeps}, pressure_iterations={}, residual={:.9e}, divergence={:.9e}, max_speed_m_s={max_speed:.9e}, changed_phase_slots={changed_cells}, elapsed_s={:.3}",
                                progress.tick,
                                pressure.iterations,
                                pressure.scaled_residual,
                                pressure.scaled_divergence,
                                started.elapsed().as_secs_f64()
                            );
                            write_checkpoint(label, progress.tick, "accepted", &accepted_bytes);
                        }
                    }
                    if progress.tick == TARGET_TICKS {
                        assert!(
                            max_speed >= 0.001 && max_changed_phase_slots >= 2,
                            "painted scene must produce physical motion"
                        );
                        return;
                    }
                    before_bytes = accepted_bytes;
                }
                Err(error) => {
                    let after_bytes = scene
                        .checkpoint_prototype()
                        .await
                        .expect("rejected checkpoint");
                    assert_eq!(
                        scene.progress(),
                        before_progress,
                        "failed candidate changed committed progress"
                    );
                    assert_eq!(
                        after_bytes, before_bytes,
                        "failed candidate changed committed fields"
                    );
                    write_checkpoint(label, before_progress.tick, "rejected", &after_bytes);
                    eprintln!(
                        "INTERACTIVE {label}: rejected_tick={}, accepted_substeps={accepted_substeps}, committed_atomicity=pass, elapsed_s={:.3}, error={error}",
                        before_progress.tick,
                        started.elapsed().as_secs_f64()
                    );
                    panic!(
                        "interactive {label} rejected at tick {}: {error}",
                        before_progress.tick
                    );
                }
            }
        }
        panic!("interactive {label} exceeded its bounded substep horizon");
    });
}

fn checkpoint_fields(bytes: &[u8]) -> [Vec<f32>; 5] {
    assert_eq!(&bytes[..4], b"PFCP");
    let mut start = 108;
    let fields = std::array::from_fn(|slot| {
        let offset = 68 + slot * size_of::<u64>();
        let count = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;
        let end = start + count * size_of::<f32>();
        let values = bytes[start..end]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|word| f32::from_le_bytes(*word))
            .collect::<Vec<_>>();
        assert!(values.iter().all(|value| value.is_finite()));
        start = end;
        values
    });
    assert_eq!(start, bytes.len());
    fields
}

fn inventory(fields: &[Vec<f32>; 5]) -> [f64; 4] {
    let cells = WIDTH * HEIGHT;
    std::array::from_fn(|slot| {
        fields[slot / 2][slot % 2 * cells..(slot % 2 + 1) * cells]
            .iter()
            .map(|&value| f64::from(value))
            .sum()
    })
}

fn assert_inventory_conserved(initial: [f64; 4], actual: [f64; 4]) {
    for (before, after) in initial.into_iter().zip(actual) {
        assert!(
            (after - before).abs() <= 1e-5 * before.abs().max(1e-12),
            "interactive paint inventory drifted"
        );
    }
}

fn write_checkpoint(label: &str, tick: u64, state: &str, bytes: &[u8]) {
    if let Some(directory) = std::env::var_os("PARTICLE_INTERACTIVE_DIAGNOSTIC_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).expect("interactive artifact directory");
        std::fs::write(
            directory.join(format!("{label}-{state}-{tick}.pfcp")),
            bytes,
        )
        .expect("interactive checkpoint artifact");
    }
}
