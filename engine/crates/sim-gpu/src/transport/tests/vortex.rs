use super::*;

#[test]
#[ignore = "requires a native GPU adapter"]
fn closed_vortex_matches_cpu_phase_and_marker_candidate() {
    let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter for E07 fixture");
    let grid = Grid::new(2.0, 2.0).unwrap();
    let cell_volume = grid.cell_volume_m3();
    let fractions = [0.1, 0.45, 0.8, 0.3];
    let liquid = fractions.map(|alpha| alpha * cell_volume * 1000.0);
    let carrier = fractions.map(|alpha| (1.0 - alpha) * cell_volume * 1.2);
    let liquid_marker = [1.0, 0.5, 2.0, 0.25];
    let carrier_marker = [0.25, 1.0, 0.5, 2.0];
    let initial = TransportInventory::new(
        grid,
        1000.0,
        1.2,
        liquid.to_vec(),
        carrier.to_vec(),
        liquid_marker.to_vec(),
        carrier_marker.to_vec(),
        vec![false; grid.cells()],
    )
    .unwrap();
    let mut u = vec![0.0; grid.u_faces()];
    let mut v = vec![0.0; grid.v_faces()];
    u[grid.u_face_index(1, 0).unwrap()] = 1.0;
    u[grid.u_face_index(1, 1).unwrap()] = -1.0;
    v[grid.v_face_index(0, 1).unwrap()] = -1.0;
    v[grid.v_face_index(1, 1).unwrap()] = 1.0;
    let mut u_aperture = vec![0.0; grid.u_faces()];
    let mut v_aperture = vec![0.0; grid.v_faces()];
    for y in 0..grid.height() {
        u_aperture[grid.u_face_index(1, y).unwrap()] = 1.0;
    }
    for x in 0..grid.width() {
        v_aperture[grid.v_face_index(x, 1).unwrap()] = 1.0;
    }
    let fields = PressureFields::new(
        grid,
        [Boundary::Closed; 4],
        vec![1000.0; grid.cells()],
        vec![0.0; grid.cells()],
        FaceValues {
            u: u.clone(),
            v: v.clone(),
        },
        FaceValues {
            u: u_aperture.clone(),
            v: v_aperture.clone(),
        },
    )
    .unwrap();
    let cpu = initial
        .candidate(
            &fields,
            &FaceValues {
                u: u.clone(),
                v: v.clone(),
            },
            0.0025,
        )
        .unwrap();

    let mass = liquid
        .into_iter()
        .chain(carrier)
        .map(|value| value as f32)
        .collect::<Vec<_>>();
    let marker = liquid_marker
        .into_iter()
        .chain(carrier_marker)
        .map(|value| value as f32)
        .collect::<Vec<_>>();
    let headers = (0..grid.cells())
        .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
        .collect::<Vec<_>>();
    let mass_buffer = buffer(&gpu.device, "fixture phase mass", &f32_bytes(&mass));
    let marker_buffer = buffer(&gpu.device, "fixture marker", &f32_bytes(&marker));
    let header_buffer = buffer(&gpu.device, "fixture headers", &headers);
    let to_f32 = |values: &[f64]| values.iter().map(|value| *value as f32).collect::<Vec<_>>();
    let u_buffer = buffer(&gpu.device, "fixture u", &f32_bytes(&to_f32(&u)));
    let v_buffer = buffer(&gpu.device, "fixture v", &f32_bytes(&to_f32(&v)));
    let u_open_buffer = buffer(
        &gpu.device,
        "fixture u aperture",
        &f32_bytes(&to_f32(&u_aperture)),
    );
    let v_open_buffer = buffer(
        &gpu.device,
        "fixture v aperture",
        &f32_bytes(&to_f32(&v_aperture)),
    );
    let input = GpuTransportInput {
        mass_kg: &mass_buffer,
        marker: &marker_buffer,
        headers: &header_buffer,
        u_velocity_m_s: &u_buffer,
        v_velocity_m_s: &v_buffer,
        u_aperture: &u_open_buffer,
        v_aperture: &v_open_buffer,
    };
    let stage = GpuTransportStage::new(&gpu.device, grid).unwrap();
    let candidate = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("transport fixture"),
        });
    stage
        .encode(
            &gpu.device,
            &mut encoder,
            &input,
            &candidate,
            GpuTransportStep {
                dt_s: 0.0025,
                liquid_density_kg_m3: 1000.0,
                carrier_density_kg_m3: 1.2,
            },
        )
        .unwrap();
    gpu.queue.submit([encoder.finish()]);
    let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
    assert_eq!(u32::from_le_bytes(status.try_into().unwrap()), 0);
    let actual_mass = read_f32(&gpu.device, &gpu.queue, &candidate.mass_kg);
    let actual_marker = read_f32(&gpu.device, &gpu.queue, &candidate.marker);
    let mut max_scaled_amount_error = 0.0_f64;
    let mut max_alpha_error = 0.0_f64;
    for cell in 0..grid.cells() {
        for (actual, expected, scale) in [
            (
                actual_mass[cell],
                cpu.inventory.liquid_mass_kg()[cell],
                0.001,
            ),
            (
                actual_mass[grid.cells() + cell],
                cpu.inventory.carrier_mass_kg()[cell],
                1.2e-6,
            ),
            (
                actual_marker[cell],
                cpu.inventory.liquid_marker()[cell],
                1.0,
            ),
            (
                actual_marker[grid.cells() + cell],
                cpu.inventory.carrier_marker()[cell],
                1.0,
            ),
        ] {
            assert!(actual >= 0.0, "cell {cell}: negative transported amount");
            max_scaled_amount_error =
                max_scaled_amount_error.max(((actual as f64 - expected) / scale).abs());
            assert!(
                ((actual as f64 - expected) / scale).abs() <= 1e-5,
                "cell {cell}: GPU {actual}, CPU {expected}"
            );
        }
        let alpha = actual_mass[cell] as f64 / 1000.0 / cell_volume;
        assert!((0.0..=1.0).contains(&alpha));
        max_alpha_error = max_alpha_error.max((alpha - cpu.inventory.alpha(cell).unwrap()).abs());
        assert!((alpha - cpu.inventory.alpha(cell).unwrap()).abs() <= 1e-5);
    }
    let total = |values: &[f32]| values.iter().map(|value| *value as f64).sum::<f64>();
    let total_liquid = total(&actual_mass[..grid.cells()]);
    let total_carrier = total(&actual_mass[grid.cells()..]);
    let total_liquid_marker = total(&actual_marker[..grid.cells()]);
    let total_carrier_marker = total(&actual_marker[grid.cells()..]);
    let before = initial.totals();
    let max_relative_total_drift = [
        (total_liquid, before.liquid_mass_kg),
        (total_carrier, before.carrier_mass_kg),
        (total_liquid_marker, before.liquid_marker),
        (total_carrier_marker, before.carrier_marker),
    ]
    .into_iter()
    .map(|(actual, expected)| ((actual - expected) / expected).abs())
    .fold(0.0_f64, f64::max);
    assert!(max_relative_total_drift <= 1e-5);
    println!(
        "GPU transport parity: max alpha error {max_alpha_error:.3e}, max scaled amount error {max_scaled_amount_error:.3e}, max relative total drift {max_relative_total_drift:.3e}"
    );
    let face_flux = read_f32(&gpu.device, &gpu.queue, &candidate.u_flux);
    let face = grid.u_face_index(1, 0).unwrap();
    assert!((face_flux[face * 5 + 1] as f64 - cpu.fluxes.liquid_mass_kg.u[face]).abs() < 1e-8);

    let rejected = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("transport excessive CFL fixture"),
        });
    stage
        .encode(
            &gpu.device,
            &mut encoder,
            &input,
            &rejected,
            GpuTransportStep {
                dt_s: 0.006,
                liquid_density_kg_m3: 1000.0,
                carrier_density_kg_m3: 1.2,
            },
        )
        .unwrap();
    gpu.queue.submit([encoder.finish()]);
    let status = read_bytes(&gpu.device, &gpu.queue, &rejected.status);
    assert_ne!(
        u32::from_le_bytes(status.try_into().unwrap()) & STATUS_INVALID_FACE,
        0
    );
    gpu.dispose();
}
