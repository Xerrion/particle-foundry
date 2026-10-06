use super::*;

#[test]
#[ignore = "requires a native Metal GPU"]
fn metal_tiny_phase_marker_flux_survives_full_and_partial_transfer() {
    let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter");
    assert_eq!(gpu.backend(), "Metal");
    let grid = Grid::new(2.0, 2.0).unwrap();
    let cells = grid.cells();
    let volume = grid.cell_volume_m3() as f32;
    let tiny_mass = 1.9347336e-21_f32;
    let tiny_marker = 4.8368337e-19_f32;
    let mass = [
        tiny_mass,
        0.0,
        1000.0 * volume,
        1000.0 * volume,
        1.2 * volume,
        1.2 * volume,
        0.0,
        0.0,
    ];
    let marker = [tiny_marker, 0.0, 0.25, 0.25, 0.05, 0.05, 0.0, 0.0];
    let headers = (0..cells)
        .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
        .collect::<Vec<_>>();
    let mass_buffer = buffer(&gpu.device, "tiny phase mass", &f32_bytes(&mass));
    let marker_buffer = buffer(&gpu.device, "tiny phase marker", &f32_bytes(&marker));
    let header_buffer = buffer(&gpu.device, "tiny phase headers", &headers);
    let mut aperture = vec![0.0_f32; grid.v_faces()];
    let face = grid.v_face_index(0, 1).unwrap();
    aperture[face] = 1.0;
    let aperture_buffer = buffer(&gpu.device, "tiny phase aperture", &f32_bytes(&aperture));
    let stage = GpuTransportStage::new(&gpu.device, grid).unwrap();
    let dt = (1.0 / 60.0) as f32;
    let mut params = [0_u8; 32];
    for (slot, value) in [grid.width(), grid.height(), cells as u32, 0]
        .into_iter()
        .enumerate()
    {
        params[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (slot, value) in [dt, 1000.0_f32, 1.2_f32, volume].into_iter().enumerate() {
        params[16 + slot * 4..20 + slot * 4].copy_from_slice(&value.to_le_bytes());
    }
    let uniform = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tiny phase face params"),
            contents: &params,
            usage: wgpu::BufferUsages::UNIFORM,
        });
    for (name, speed, expected_fraction) in [
        ("full", 6.512966e-17_f32, 1.0_f64),
        ("partial", 5e-19_f32, 0.4307_f64),
    ] {
        let mut velocity = vec![0.0_f32; grid.v_faces()];
        velocity[face] = speed;
        let velocity_buffer = buffer(&gpu.device, "tiny phase v", &f32_bytes(&velocity));
        let candidate = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
        let bindings = face_group(
            &gpu.device,
            &stage.y_face,
            &uniform,
            &mass_buffer,
            &marker_buffer,
            &header_buffer,
            &velocity_buffer,
            &aperture_buffer,
            &candidate.v_flux,
            &candidate.status,
        );
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tiny phase marker flux"),
            });
        encoder.clear_buffer(&candidate.status, 0, None);
        run_pass(&mut encoder, &stage.y_face, &bindings, grid.v_faces());
        gpu.queue.submit([encoder.finish()]);
        let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
        assert_eq!(u32::from_le_bytes(status.try_into().unwrap()), 0);
        let flux = read_f32(&gpu.device, &gpu.queue, &candidate.v_flux);
        let moved_mass = flux[face * 5 + 1];
        let moved_marker = flux[face * 5 + 3];
        let moved_fraction = f64::from(moved_mass) / f64::from(tiny_mass);
        assert!(
            (moved_fraction - expected_fraction).abs() < 1e-3,
            "{name} mass transfer fraction {moved_fraction}"
        );
        let expected_marker = f64::from(tiny_marker) * moved_fraction;
        assert!(
            (f64::from(moved_marker) - expected_marker).abs() <= expected_marker * 1e-5,
            "{name} marker transfer: expected {expected_marker:e}, got {moved_marker:e}"
        );
    }
    gpu.dispose();
}
