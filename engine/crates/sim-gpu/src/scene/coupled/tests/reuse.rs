use super::*;

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn poisoned_scratch_and_cancelled_submissions_retry_like_a_fresh_scene() {
    let context = block_on(GpuContext::new()).unwrap();
    for pause in [
        TestFaultPoint::SubmittedStages,
        TestFaultPoint::BeforeCommit,
    ] {
        let physics = || GpuScenePhysics {
            cell_dynamic_viscosity_pa_s: Some(vec![0.001; 4]),
            gravity_m_s2: 0.0,
            pressure: PressureConfig::default(),
        };
        let mut cancelled = GpuCoupledScene::new(&context, mixed_vortex_seed(), physics()).unwrap();
        let mut fresh = GpuCoupledScene::new(&context, mixed_vortex_seed(), physics()).unwrap();
        let before = committed_metadata(&cancelled);
        let committed = block_on(read_fixture_committed_state(&cancelled.scene)).unwrap();
        poison_scratch(&context, &cancelled.candidates);
        cancelled.test_fault = Some(pause);
        let waker: Waker = Arc::new(Notify(std::thread::current())).into();
        let mut cx = Context::from_waker(&waker);
        let mut pending =
            Box::pin(cancelled.advance_candidate(0.001, 0.001, 0.0, PressureConfig::default()));
        assert!(pending.as_mut().poll(&mut cx).is_pending(), "{pause:?}");
        drop(pending);
        cancelled.test_fault = None;
        assert_eq!(committed_metadata(&cancelled), before);
        assert_eq!(
            block_on(read_fixture_committed_state(&cancelled.scene)).unwrap(),
            committed
        );
        // Repeated overwrites cover reuse after cancellation and after acceptance.
        for _ in 0..3 {
            poison_scratch(&context, &cancelled.candidates);
            for scene in [&mut cancelled, &mut fresh] {
                block_on(scene.advance_candidate(0.001, 0.001, 0.0, PressureConfig::default()))
                    .unwrap();
            }
            assert_eq!(committed_metadata(&cancelled), committed_metadata(&fresh));
            assert_eq!(
                block_on(read_fixture_committed_state(&cancelled.scene)).unwrap(),
                block_on(read_fixture_committed_state(&fresh.scene)).unwrap(),
                "{pause:?}"
            );
        }
    }
    context.dispose();
}

fn poison_scratch(context: &GpuContext, candidate: &CandidateBuffers) {
    let shader = context
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("candidate poison fixture"),
            source: wgpu::ShaderSource::Wgsl(
                "@group(0) @binding(0) var<storage, read_write> data: array<u32>;
             @compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) {
                if id.x < arrayLength(&data) { data[id.x] = 0x7fc00000u; }
             }"
                .into(),
            ),
        });
    let pipeline = context
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("candidate poison fixture"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
    let t = &candidate.transport;
    let m = &candidate.momentum;
    let g = &candidate.gravity;
    let mut fields = vec![
        &t.base_residual,
        &t.closed_u_velocity_m_s,
        &t.closed_v_velocity_m_s,
        &t.after_x_mass_kg,
        &t.after_x_marker,
        &t.mass_kg,
        &t.marker,
        &t.provisional_u_flux,
        &t.provisional_v_flux,
        &t.u_flux,
        &t.v_flux,
        &t.status,
        &m.after_x_u_velocity_m_s,
        &m.after_x_v_velocity_m_s,
        &m.u_velocity_m_s,
        &m.v_velocity_m_s,
        &m.ux_dual_flux,
        &m.vx_dual_flux,
        &m.uy_dual_flux,
        &m.vy_dual_flux,
        &m.ux_wall_impulse,
        &m.vx_wall_impulse,
        &m.uy_wall_impulse,
        &m.vy_wall_impulse,
        &m.status,
        &candidate.density.density_kg_m3,
        &candidate.density.status,
        &g.u_velocity_m_s,
        &g.v_velocity_m_s,
        &g.status,
    ];
    if let Some(v) = &candidate.viscosity {
        fields.extend([
            &v.u_velocity_m_s,
            &v.v_velocity_m_s,
            &v.corner_stress_pa,
            &v.status,
        ]);
    }
    let mut encoder = context.device.create_command_encoder(&Default::default());
    for field in fields {
        let group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("candidate poison fixture"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: field.as_entire_binding(),
                }],
            });
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups((field.size() as u32 / 4).div_ceil(64), 1, 1);
    }
    context.queue.submit([encoder.finish()]);
}
