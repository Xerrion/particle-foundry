use super::*;

#[test]
#[ignore = "requires a native Metal GPU"]
fn metal_y_budget_uses_inventory_after_x_transfer() {
    let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter");
    assert_eq!(gpu.backend(), "Metal");
    let grid = Grid::new(3.0, 3.0).unwrap();
    let cells = grid.cells();
    let volume = grid.cell_volume_m3() as f32;
    let water_unit = 1000.0 * volume;
    let air_unit = 1.2 * volume;
    let center = grid.cell_index(1, 1).unwrap();
    let left = grid.cell_index(0, 1).unwrap();
    let right = grid.cell_index(2, 1).unwrap();
    let top = grid.cell_index(1, 0).unwrap();
    let bottom = grid.cell_index(1, 2).unwrap();
    let mut liquid = vec![0.0_f32; cells];
    let mut carrier = vec![air_unit; cells];
    let mut liquid_marker = vec![0.0_f32; cells];
    let mut carrier_marker = vec![0.25_f32; cells];
    liquid[left] = 1.25 * water_unit;
    carrier[left] = 0.0;
    liquid_marker[left] = 2.5;
    carrier_marker[left] = 0.0;
    liquid[center] = 0.1 * water_unit;
    carrier[center] = 0.9 * air_unit;
    liquid_marker[center] = 0.2;
    carrier_marker[center] = 0.9;
    carrier[right] = 1.25 * air_unit;
    carrier_marker[right] = 0.5;
    carrier[top] = 0.75 * air_unit;
    carrier[bottom] = 0.75 * air_unit;
    let mass = liquid.into_iter().chain(carrier).collect::<Vec<_>>();
    let marker = liquid_marker
        .into_iter()
        .chain(carrier_marker)
        .collect::<Vec<_>>();
    let headers = (0..cells)
        .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
        .collect::<Vec<_>>();
    let dt = 0.01_f32;
    let face_speed = 0.25 * CELL_WIDTH_M as f32 / dt;
    let left_face = grid.u_face_index(1, 1).unwrap();
    let right_face = grid.u_face_index(2, 1).unwrap();
    let top_face = grid.v_face_index(1, 1).unwrap();
    let bottom_face = grid.v_face_index(1, 2).unwrap();
    let mut u = vec![0.0_f32; grid.u_faces()];
    let mut v = vec![0.0_f32; grid.v_faces()];
    let mut u_open = vec![0.0_f32; grid.u_faces()];
    let mut v_open = vec![0.0_f32; grid.v_faces()];
    u[left_face] = face_speed;
    u[right_face] = -face_speed;
    v[top_face] = -face_speed;
    v[bottom_face] = face_speed;
    u_open[left_face] = 1.0;
    u_open[right_face] = 1.0;
    v_open[top_face] = 1.0;
    v_open[bottom_face] = 1.0;
    let mass_buffer = buffer(&gpu.device, "x then y phase mass", &f32_bytes(&mass));
    let marker_buffer = buffer(&gpu.device, "x then y marker", &f32_bytes(&marker));
    let header_buffer = buffer(&gpu.device, "x then y headers", &headers);
    let u_buffer = buffer(&gpu.device, "x then y horizontal speed", &f32_bytes(&u));
    let v_buffer = buffer(&gpu.device, "x then y vertical speed", &f32_bytes(&v));
    let u_open_buffer = buffer(
        &gpu.device,
        "x then y horizontal aperture",
        &f32_bytes(&u_open),
    );
    let v_open_buffer = buffer(
        &gpu.device,
        "x then y vertical aperture",
        &f32_bytes(&v_open),
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
            label: Some("x then y donor budget fixture"),
        });
    stage
        .encode(
            &gpu.device,
            &mut encoder,
            &input,
            &candidate,
            GpuTransportStep {
                dt_s: dt,
                liquid_density_kg_m3: 1000.0,
                carrier_density_kg_m3: 1.2,
            },
        )
        .unwrap();
    gpu.queue.submit([encoder.finish()]);
    let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
    assert_eq!(u32::from_le_bytes(status.try_into().unwrap()), 0);

    let after_x_mass = read_f32(&gpu.device, &gpu.queue, &candidate.after_x_mass_kg);
    let after_x_marker = read_f32(&gpu.device, &gpu.queue, &candidate.after_x_marker);
    assert!(after_x_mass[center] > mass[center] + 0.2 * water_unit);
    assert!(after_x_marker[center] > marker[center] + 0.4);

    let provisional_y = read_f32(&gpu.device, &gpu.queue, &candidate.provisional_v_flux);
    let corrected_y = read_f32(&gpu.device, &gpu.queue, &candidate.v_flux);
    let top_liquid = -corrected_y[top_face * 5 + 1];
    let bottom_liquid = corrected_y[bottom_face * 5 + 1];
    let top_marker = -corrected_y[top_face * 5 + 3];
    let bottom_marker = corrected_y[bottom_face * 5 + 3];
    assert!(top_liquid > 0.0 && bottom_liquid > 0.0);
    assert!(top_marker > 0.0 && bottom_marker > 0.0);
    assert!(
        top_liquid + bottom_liquid > mass[center] * 1.05,
        "the two Y outlets must exceed the pre-X liquid budget"
    );
    assert!(top_liquid + bottom_liquid < after_x_mass[center]);
    for face in [top_face, bottom_face] {
        // No phase overdraw changes the shared volume or mass ledger.
        for slot in 0..3 {
            assert_eq!(
                corrected_y[face * 5 + slot],
                provisional_y[face * 5 + slot],
                "Y face {face} slot {slot} changed despite the after-X budget"
            );
        }
        // Joint marker allocation can round one face by one f32 ULP.
        for slot in 3..5 {
            let expected = provisional_y[face * 5 + slot];
            assert!(
                (corrected_y[face * 5 + slot] - expected).abs()
                    <= expected.abs() * (2.0 * f32::EPSILON),
                "Y face {face} marker {slot} changed its transported concentration"
            );
        }
    }

    let result_mass = read_f32(&gpu.device, &gpu.queue, &candidate.mass_kg);
    let result_marker = read_f32(&gpu.device, &gpu.queue, &candidate.marker);
    for cell in 0..cells {
        let fill = result_mass[cell] / 1000.0 / volume + result_mass[cells + cell] / 1.2 / volume;
        assert!((fill - 1.0).abs() <= 1e-5, "cell {cell} fill {fill}");
    }
    let total = |values: &[f32]| values.iter().map(|&value| f64::from(value)).sum::<f64>();
    for (before, after) in [
        (&mass[..cells], &result_mass[..cells]),
        (&mass[cells..], &result_mass[cells..]),
        (&marker[..cells], &result_marker[..cells]),
        (&marker[cells..], &result_marker[cells..]),
    ] {
        let expected = total(before);
        let actual = total(after);
        assert!(
            (actual - expected).abs() <= expected * 1e-6,
            "transport total changed from {expected:e} to {actual:e}"
        );
    }
    gpu.dispose();
}
