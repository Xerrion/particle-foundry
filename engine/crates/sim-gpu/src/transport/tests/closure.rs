use super::*;

#[test]
#[ignore = "requires a native GPU adapter"]
fn local_shared_face_correction_closes_without_losing_phase_or_marker() {
    let gpu = block_on(crate::GpuContext::new()).expect("native GPU transport adapter");
    let grid = Grid::new(2.0, 2.0).unwrap();
    let volume = grid.cell_volume_m3() as f32;
    let water_mass = 1000.0 * volume;
    let mass = [
        water_mass * (1.0 - 8e-6),
        water_mass * (1.0 + 8e-6),
        water_mass,
        water_mass,
        0.0,
        0.0,
        0.0,
        0.0,
    ];
    let marker = [0.25_f32, 0.5, 0.75, 1.0, 0.0, 0.0, 0.0, 0.0];
    let headers = (0..grid.cells())
        .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
        .collect::<Vec<_>>();
    let mut u = vec![0.0_f32; grid.u_faces()];
    u[grid.u_face_index(1, 0).unwrap()] = 5e-6;
    let v = vec![0.0_f32; grid.v_faces()];
    let mut u_aperture = vec![0.0_f32; grid.u_faces()];
    let mut v_aperture = vec![0.0_f32; grid.v_faces()];
    for y in 0..grid.height() {
        u_aperture[grid.u_face_index(1, y).unwrap()] = 1.0;
    }
    for x in 0..grid.width() {
        v_aperture[grid.v_face_index(x, 1).unwrap()] = 1.0;
    }
    let mass_buffer = buffer(&gpu.device, "imbalanced water mass", &f32_bytes(&mass));
    let marker_buffer = buffer(&gpu.device, "imbalanced marker", &f32_bytes(&marker));
    let header_buffer = buffer(&gpu.device, "open cell headers", &headers);
    let u_buffer = buffer(&gpu.device, "small divergent u", &f32_bytes(&u));
    let v_buffer = buffer(&gpu.device, "zero v", &f32_bytes(&v));
    let u_open_buffer = buffer(&gpu.device, "open u faces", &f32_bytes(&u_aperture));
    let v_open_buffer = buffer(&gpu.device, "open v faces", &f32_bytes(&v_aperture));
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
            label: Some("conservative local closure fixture"),
        });
    stage
        .encode(
            &gpu.device,
            &mut encoder,
            &input,
            &candidate,
            GpuTransportStep {
                dt_s: 0.01,
                liquid_density_kg_m3: 1000.0,
                carrier_density_kg_m3: 1.2,
            },
        )
        .unwrap();
    gpu.queue.submit([encoder.finish()]);
    let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
    assert_eq!(u32::from_le_bytes(status.try_into().unwrap()), 0);
    let result_mass = read_f32(&gpu.device, &gpu.queue, &candidate.mass_kg);
    let result_marker = read_f32(&gpu.device, &gpu.queue, &candidate.marker);
    let face_flux = read_f32(&gpu.device, &gpu.queue, &candidate.u_flux);
    let face = grid.u_face_index(1, 0).unwrap();
    let source_fill = mass[0] / water_mass;
    let uncorrected_fill = source_fill - u[face] * 0.01 / CELL_WIDTH_M as f32;
    assert!(
        (uncorrected_fill - 1.0).abs() > 1e-5,
        "uncorrected candidate must cross the closure gate"
    );
    for cell in 0..grid.cells() {
        let fill = result_mass[cell] / 1000.0 / volume;
        assert!((fill - 1.0).abs() < 3e-6, "cell {cell} fill {fill}");
        assert_eq!(result_mass[grid.cells() + cell], 0.0);
        assert_eq!(result_marker[grid.cells() + cell], 0.0);
    }
    let sum = |values: &[f32]| values.iter().map(|&value| f64::from(value)).sum::<f64>();
    let before_mass = sum(&mass[..grid.cells()]);
    let after_mass = sum(&result_mass[..grid.cells()]);
    let before_marker = sum(&marker[..grid.cells()]);
    let after_marker = sum(&result_marker[..grid.cells()]);
    assert!((after_mass - before_mass).abs() <= before_mass * 1e-7);
    assert!((after_marker - before_marker).abs() <= before_marker * 1e-7);
    assert!(
        face_flux[face * 5] < 0.0,
        "correction reverses the excess outflow"
    );
    assert!((face_flux[face * 5 + 1] / 1000.0 - face_flux[face * 5]).abs() <= 1e-13);
    gpu.dispose();
}
