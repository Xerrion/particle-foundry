use super::*;

#[test]
#[ignore = "requires a native Metal GPU"]
fn metal_balanced_gather_resolves_small_opposing_fluxes() {
    let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter");
    assert_eq!(gpu.backend(), "Metal");
    let grid = Grid::new(3.0, 1.0).unwrap();
    let center_mass = f32::from_bits(0x35a1_1016);
    let outgoing = f32::from_bits(0x2d2c_d096);
    let incoming = f32::from_bits(0x2d2f_b912);
    let expected = (f64::from(center_mass) - f64::from(outgoing) + f64::from(incoming)) as f32;
    assert_eq!(expected.to_bits(), 0x35a1_1017);
    assert_eq!(((center_mass - outgoing) + incoming).to_bits(), 0x35a1_1018);
    let carrier_unit = 1.2_f32 * grid.cell_volume_m3() as f32;
    let mass = [0.0_f32, 0.0, 0.0, carrier_unit, center_mass, carrier_unit];
    let marker = [0.0_f32; 6];
    let headers = (0..grid.cells())
        .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
        .collect::<Vec<_>>();
    let mass_buffer = buffer(&gpu.device, "balanced gather mass", &f32_bytes(&mass));
    let marker_buffer = buffer(&gpu.device, "balanced gather marker", &f32_bytes(&marker));
    let header_buffer = buffer(&gpu.device, "balanced gather headers", &headers);
    let mut x_flux = vec![0.0_f32; grid.u_faces() * 5];
    for (face, flow) in [(1_usize, outgoing), (2_usize, incoming)] {
        x_flux[face * 5] = -flow / 1.2;
        x_flux[face * 5 + 2] = -flow;
    }
    let x_flux_buffer = buffer(&gpu.device, "balanced gather X flux", &f32_bytes(&x_flux));
    let y_flux_buffer = buffer(
        &gpu.device,
        "balanced gather zero Y flux",
        &f32_bytes(&vec![0.0_f32; grid.v_faces() * 5]),
    );
    let mut params = [0_u8; 32];
    for (slot, value) in [grid.width(), grid.height(), grid.cells() as u32, 0]
        .into_iter()
        .enumerate()
    {
        params[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (slot, value) in [1.0_f32 / 60.0, 1000.0, 1.2, grid.cell_volume_m3() as f32]
        .into_iter()
        .enumerate()
    {
        params[16 + slot * 4..20 + slot * 4].copy_from_slice(&value.to_le_bytes());
    }
    let uniform = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("balanced gather parameters"),
            contents: &params,
            usage: wgpu::BufferUsages::UNIFORM,
        });
    let stage = GpuTransportStage::new(&gpu.device, grid).unwrap();
    let candidate = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
    let x_bindings = gather_group(
        &gpu.device,
        &stage.x_gather,
        &uniform,
        &mass_buffer,
        &marker_buffer,
        &header_buffer,
        &x_flux_buffer,
        &candidate.after_x_mass_kg,
        &candidate.after_x_marker,
        &candidate.status,
    );
    let y_bindings = gather_group(
        &gpu.device,
        &stage.y_gather,
        &uniform,
        &candidate.after_x_mass_kg,
        &candidate.after_x_marker,
        &header_buffer,
        &y_flux_buffer,
        &candidate.mass_kg,
        &candidate.marker,
        &candidate.status,
    );
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("balanced gather cancellation fixture"),
        });
    encoder.clear_buffer(&candidate.status, 0, None);
    run_pass(&mut encoder, &stage.x_gather, &x_bindings, grid.cells());
    // The zero Y sweep applies the unchanged final volume gate directly.
    // No closure correction can hide rounding introduced by the X sweep.
    run_pass(&mut encoder, &stage.y_gather, &y_bindings, grid.cells());
    gpu.queue.submit([encoder.finish()]);
    let after_x = read_f32(&gpu.device, &gpu.queue, &candidate.after_x_mass_kg);
    assert_eq!(after_x[4].to_bits(), expected.to_bits());
    let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
    assert_eq!(u32::from_le_bytes(status.try_into().unwrap()), 0);
    let result_mass = read_f32(&gpu.device, &gpu.queue, &candidate.mass_kg);
    let result_marker = read_f32(&gpu.device, &gpu.queue, &candidate.marker);
    assert_eq!(result_mass, after_x);
    assert!(
        result_mass
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
    );
    assert!(result_mass[..3].iter().all(|value| *value == 0.0));
    assert_eq!(result_marker, marker);
    let before: f64 = mass[3..].iter().map(|value| f64::from(*value)).sum();
    let after: f64 = result_mass[3..].iter().map(|value| f64::from(*value)).sum();
    assert!((after - before).abs() <= before * 2e-7);
    gpu.dispose();
}

#[test]
#[ignore = "requires a native Metal GPU"]
fn metal_exhausted_gather_retains_small_incoming_phase_and_marker_on_both_axes() {
    let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter");
    assert_eq!(gpu.backend(), "Metal");
    let owned_phase = f32::from_bits(0x2582_b62b);
    let owned_marker = f32::from_bits(0x297f_4bc5);
    let small_phase = f32::from_bits(0x195d_dc65);
    let small_marker = f32::from_bits(0x1d58_a937);
    // Adding the incoming amount first loses the phase and changes the
    // marker to one inventory ULP. Subtraction first retains both inputs.
    assert_eq!(((owned_phase + small_phase) - owned_phase).to_bits(), 0);
    assert_eq!(
        ((owned_marker + small_marker) - owned_marker).to_bits(),
        0x1d80_0000
    );
    for (axis, grid) in [
        (0_u32, Grid::new(3.0, 1.0).unwrap()),
        (1_u32, Grid::new(1.0, 3.0).unwrap()),
    ] {
        let stage = GpuTransportStage::new(&gpu.device, grid).unwrap();
        let headers = (0..grid.cells())
            .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
            .collect::<Vec<_>>();
        let header_buffer = buffer(&gpu.device, "exhausted gather headers", &headers);
        let carrier_unit = 1.2_f32 * grid.cell_volume_m3() as f32;
        let mut params = [0_u8; 32];
        for (slot, value) in [grid.width(), grid.height(), grid.cells() as u32, axis]
            .into_iter()
            .enumerate()
        {
            params[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (slot, value) in [1.0_f32 / 60.0, 1000.0, 1.2, grid.cell_volume_m3() as f32]
            .into_iter()
            .enumerate()
        {
            params[16 + slot * 4..20 + slot * 4].copy_from_slice(&value.to_le_bytes());
        }
        let uniform = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("exhausted gather parameters"),
                contents: &params,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        for (incoming_phase, incoming_marker) in [(small_phase, small_marker), (0.0_f32, 0.0_f32)] {
            let mass = [
                incoming_phase,
                owned_phase,
                0.0,
                carrier_unit,
                carrier_unit,
                carrier_unit,
            ];
            let marker = [incoming_marker, owned_marker, 0.0, 0.0, 0.0, 0.0];
            let mass_buffer = buffer(&gpu.device, "exhausted gather mass", &f32_bytes(&mass));
            let marker_buffer = buffer(&gpu.device, "exhausted gather marker", &f32_bytes(&marker));
            let mut flux = vec![0.0_f32; 4 * 5];
            for (face, phase, associated_marker) in [
                (1_usize, incoming_phase, incoming_marker),
                (2_usize, owned_phase, owned_marker),
            ] {
                flux[face * 5] = phase / 1000.0;
                flux[face * 5 + 1] = phase;
                flux[face * 5 + 3] = associated_marker;
            }
            let flux_buffer = buffer(&gpu.device, "exhausted gather flux", &f32_bytes(&flux));
            let candidate = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
            let (pipeline, output_mass, output_marker) = if axis == 0 {
                (
                    &stage.x_gather,
                    &candidate.after_x_mass_kg,
                    &candidate.after_x_marker,
                )
            } else {
                (&stage.y_gather, &candidate.mass_kg, &candidate.marker)
            };
            let bindings = gather_group(
                &gpu.device,
                pipeline,
                &uniform,
                &mass_buffer,
                &marker_buffer,
                &header_buffer,
                &flux_buffer,
                output_mass,
                output_marker,
                &candidate.status,
            );
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("exhausted gather incoming fixture"),
                });
            encoder.clear_buffer(&candidate.status, 0, None);
            run_pass(&mut encoder, pipeline, &bindings, grid.cells());
            gpu.queue.submit([encoder.finish()]);
            let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
            assert_eq!(
                u32::from_le_bytes(status.try_into().unwrap()),
                0,
                "axis {axis}"
            );
            let result_mass = read_f32(&gpu.device, &gpu.queue, output_mass);
            let result_marker = read_f32(&gpu.device, &gpu.queue, output_marker);
            for (actual, expected) in result_mass[..3]
                .iter()
                .zip([0.0_f32, incoming_phase, owned_phase])
                .chain(
                    result_marker[..3]
                        .iter()
                        .zip([0.0_f32, incoming_marker, owned_marker]),
                )
            {
                assert_eq!(actual.to_bits(), expected.to_bits(), "axis {axis}");
            }
            assert_eq!(&result_mass[3..], &mass[3..]);
            assert_eq!(&result_marker[3..], &marker[3..]);
            assert!(
                result_mass
                    .iter()
                    .chain(&result_marker)
                    .all(|value| value.is_finite() && *value >= 0.0)
            );
            for (before, after) in [
                (&mass[..3], &result_mass[..3]),
                (&mass[3..], &result_mass[3..]),
                (&marker[..3], &result_marker[..3]),
                (&marker[3..], &result_marker[3..]),
            ] {
                let before: f64 = before.iter().map(|value| f64::from(*value)).sum();
                let after: f64 = after.iter().map(|value| f64::from(*value)).sum();
                assert!(
                    (after - before).abs() <= before * 4.0 * f64::EPSILON,
                    "axis {axis}: inventory drifted from {before:e} to {after:e}"
                );
            }
        }
    }
    gpu.dispose();
}
