mod clock;
mod diagnostics;
mod parity;
mod rejection;
mod reuse;
mod transactions;

use super::*;
use particle_sim::contracts::Boundary;
use particle_sim_cpu::{
    ReferenceSession,
    coupled::{CoupledStepConfig, CoupledStepOutcome},
    fluid::{FaceValues, PressureFields as CpuPressureFields},
    solver::SolveConfig,
    substep::SubstepClock,
    transport::TransportInventory,
};
use std::{
    future::Future,
    task::{Context, Wake},
    time::{Duration, Instant},
};

fn uniform_air_seed() -> GpuSceneSeed {
    e07_uniform_air_seed()
}

fn mixed_vortex_seed() -> GpuSceneSeed {
    e07_mixed_vortex_seed()
}

fn cpu_mixed_vortex_step(seed: &GpuSceneSeed) -> ReferenceSession {
    let cells = seed.grid.cells();
    let inventory = TransportInventory::new(
        seed.grid,
        seed.phase_density_kg_m3[0],
        seed.phase_density_kg_m3[1],
        seed.mass_kg[..cells].to_vec(),
        seed.mass_kg[cells..].to_vec(),
        seed.marker[..cells].to_vec(),
        seed.marker[cells..].to_vec(),
        vec![false; cells],
    )
    .unwrap();
    let density = (0..cells)
        .map(|cell| (seed.mass_kg[cell] + seed.mass_kg[cells + cell]) / seed.grid.cell_volume_m3())
        .collect();
    let fields = CpuPressureFields::new(
        seed.grid,
        [Boundary::Closed; 4],
        density,
        vec![0.0; cells],
        FaceValues {
            u: seed.u_velocity_m_s.clone(),
            v: seed.v_velocity_m_s.clone(),
        },
        FaceValues {
            u: seed.u_aperture.clone(),
            v: seed.v_aperture.clone(),
        },
    )
    .unwrap();
    let mut session = ReferenceSession::new(seed.grid);
    session.set_pressure_fields(fields).unwrap();
    session.set_transport_inventory(inventory).unwrap();
    let viscosity = vec![0.001; cells];
    let mut clock = SubstepClock::new(OUTER_DT_S, 8).unwrap();
    let outcome = session
        .advance_coupled_substep(
            &mut clock,
            CoupledStepConfig {
                gravity_m_s2: 0.0,
                cell_dynamic_viscosity_pa_s: Some(&viscosity),
                pressure: SolveConfig::default(),
            },
        )
        .unwrap();
    assert!(matches!(outcome, CoupledStepOutcome::Advanced { .. }));
    assert!((clock.accepted_time_s() - OUTER_DT_S).abs() < 1e-14);
    session
}

fn read_f32(context: &GpuContext, source: &wgpu::Buffer) -> Vec<f32> {
    let staging = context.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("coupled parity test readback"),
        size: source.size(),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = context
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("coupled parity test copy"),
        });
    encoder.copy_buffer_to_buffer(source, 0, &staging, 0, source.size());
    context.queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    staging.map_async(wgpu::MapMode::Read, .., move |result| {
        sender.send(result).unwrap();
    });
    context
        .device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(10)),
        })
        .unwrap();
    receiver
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .unwrap();
    let mapped = staging.get_mapped_range(..);
    let values = mapped
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| f32::from_le_bytes(*bytes))
        .collect();
    drop(mapped);
    staging.unmap();
    values
}

fn committed_metadata(scene: &GpuCoupledScene) -> (u64, usize, u64, f64, f64, f64) {
    (
        scene.scene.epoch,
        scene.scene.committed_index,
        scene.scene.tick,
        scene.scene.accepted_time_s,
        scene.encoded_time_s,
        scene.remaining_outer_s,
    )
}

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
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => {
                assert!(Instant::now() < deadline, "GPU coupled test timed out");
                std::thread::park_timeout(Duration::from_millis(100));
            }
        }
    }
}
