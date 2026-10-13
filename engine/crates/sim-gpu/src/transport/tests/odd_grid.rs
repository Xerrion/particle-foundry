use super::*;

#[test]
#[ignore = "requires a native GPU adapter"]
fn odd_and_single_axis_grids_close_every_cell_and_preserve_inventory() {
    let gpu = block_on(crate::GpuContext::new()).expect("native GPU transport adapter");
    for (width, height) in [(5, 3), (1, 5), (5, 1)] {
        let grid = Grid::new(f64::from(width), f64::from(height)).unwrap();
        let cells = grid.cells();
        let volume = grid.cell_volume_m3() as f32;
        let water_mass = 1000.0 * volume;
        let mut mass = vec![0.0; 2 * cells];
        let mut marker = vec![0.0; 2 * cells];
        for cell in 0..cells {
            let imbalance = match cell.cmp(&(cells / 2)) {
                std::cmp::Ordering::Less => -8e-6,
                std::cmp::Ordering::Equal => 0.0,
                std::cmp::Ordering::Greater => 8e-6,
            };
            mass[cell] = water_mass * (1.0 + imbalance);
            marker[cell] = 0.2 + 0.6 * cell as f32 / cells as f32;
        }
        let headers = (0..cells)
            .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
            .collect::<Vec<_>>();
        let mut u = vec![0.0; grid.u_faces()];
        let mut v = vec![0.0; grid.v_faces()];
        let mut u_open = vec![0.0; grid.u_faces()];
        let mut v_open = vec![0.0; grid.v_faces()];
        for y in 0..height {
            for x in 1..width {
                u_open[grid.u_face_index(x, y).unwrap()] = 1.0;
            }
        }
        for y in 1..height {
            for x in 0..width {
                v_open[grid.v_face_index(x, y).unwrap()] = 1.0;
            }
        }
        if width > 1 {
            u[grid.u_face_index(1, 0).unwrap()] = 5e-6;
        } else {
            v[grid.v_face_index(0, 1).unwrap()] = 5e-6;
        }
        let mass_buffer = buffer(&gpu.device, "odd-grid mass", &f32_bytes(&mass));
        let marker_buffer = buffer(&gpu.device, "odd-grid marker", &f32_bytes(&marker));
        let header_buffer = buffer(&gpu.device, "odd-grid headers", &headers);
        let u_buffer = buffer(&gpu.device, "odd-grid u", &f32_bytes(&u));
        let v_buffer = buffer(&gpu.device, "odd-grid v", &f32_bytes(&v));
        let u_open_buffer = buffer(&gpu.device, "odd-grid u aperture", &f32_bytes(&u_open));
        let v_open_buffer = buffer(&gpu.device, "odd-grid v aperture", &f32_bytes(&v_open));
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
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
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
        let after_mass = read_f32(&gpu.device, &gpu.queue, &candidate.mass_kg);
        let after_marker = read_f32(&gpu.device, &gpu.queue, &candidate.marker);
        for (cell, mass) in after_mass[..cells].iter().enumerate() {
            let fill = mass / 1000.0 / volume;
            assert!(
                (fill - 1.0).abs() < 3e-6,
                "{width} x {height}, cell {cell}: fill {fill}"
            );
        }
        for values in [&after_mass, &after_marker] {
            assert!(values.iter().all(|v| v.is_finite() && *v >= 0.0));
            assert!(values[cells..].iter().all(|v| *v == 0.0));
        }
        let total = |values: &[f32]| values.iter().map(|&v| f64::from(v)).sum::<f64>();
        assert!((total(&after_mass) - total(&mass)).abs() < total(&mass) * 3e-7);
        assert!((total(&after_marker) - total(&marker)).abs() < total(&marker) * 3e-7);
    }
    gpu.dispose();
}
