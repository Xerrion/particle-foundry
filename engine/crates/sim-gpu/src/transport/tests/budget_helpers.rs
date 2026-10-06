use super::*;

pub(super) fn assert_two_outlet_budget(
    gpu: &crate::GpuContext,
    mass: &[f32; 6],
    marker: &[f32; 6],
    provisional: &[f32],
    exhausts_carrier: bool,
    intermediate_sweep: bool,
) {
    let headers = (0..3)
        .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
        .collect::<Vec<_>>();
    let mass_buffer = buffer(&gpu.device, "two-outlet phase mass", &f32_bytes(mass));
    let marker_buffer = buffer(&gpu.device, "two-outlet marker", &f32_bytes(marker));
    let header_buffer = buffer(&gpu.device, "two-outlet headers", &headers);
    let provisional_buffer = buffer(
        &gpu.device,
        "two-outlet provisional ledger",
        &f32_bytes(provisional),
    );
    for (axis, grid) in [
        (0_u32, Grid::new(3.0, 1.0).unwrap()),
        (1_u32, Grid::new(1.0, 3.0).unwrap()),
    ] {
        let stage = GpuTransportStage::new(&gpu.device, grid).unwrap();
        let candidate = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
        let corrected = if axis == 0 {
            &candidate.u_flux
        } else {
            &candidate.v_flux
        };
        let (gather_pipeline, output_mass, output_marker) = if axis == 0 {
            (
                &stage.x_gather,
                &candidate.after_x_mass_kg,
                &candidate.after_x_marker,
            )
        } else {
            (&stage.y_gather, &candidate.mass_kg, &candidate.marker)
        };
        let mut params = [0_u8; 32];
        for (slot, value) in [grid.width(), grid.height(), 3, axis]
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
                label: Some("two-outlet budget parameters"),
                contents: &params,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let budget_bindings = budget_group(
            &gpu.device,
            &stage.budget,
            &uniform,
            &mass_buffer,
            &marker_buffer,
            &provisional_buffer,
            corrected,
            &candidate.status,
        );
        let gather_bindings = gather_group(
            &gpu.device,
            gather_pipeline,
            &uniform,
            &mass_buffer,
            &marker_buffer,
            &header_buffer,
            corrected,
            output_mass,
            output_marker,
            &candidate.status,
        );
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("two-outlet donor budget fixture"),
            });
        encoder.clear_buffer(&candidate.status, 0, None);
        run_pass(&mut encoder, &stage.budget, &budget_bindings, 4);
        gpu.queue.submit([encoder.finish()]);
        let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
        assert_eq!(
            u32::from_le_bytes(status.try_into().unwrap()),
            0,
            "axis {axis}: donor budget"
        );
        let flux = read_f32(&gpu.device, &gpu.queue, corrected);
        let negative_carrier = -flux[5 + 2];
        let positive_carrier = flux[10 + 2];
        let negative_marker = -flux[5 + 4];
        let positive_marker = flux[10 + 4];
        assert_eq!(flux[5], provisional[5]);
        assert_eq!(flux[10], provisional[10]);
        assert!(
            negative_carrier > 0.0 && positive_carrier > 0.0,
            "axis {axis}"
        );
        assert!(
            negative_marker > 0.0 && positive_marker > 0.0,
            "axis {axis}"
        );
        assert!(
            ((negative_carrier + positive_carrier)
                - (-provisional[5 + 2] + provisional[10 + 2]).min(mass[4]))
            .abs()
                <= mass[4] * 2e-7,
            "axis {axis}: carrier budget"
        );
        if exhausts_carrier {
            assert_eq!(negative_carrier + positive_carrier, mass[4]);
            assert_eq!(negative_marker + positive_marker, marker[4]);
        } else {
            let expected_marker = marker[4] * ((negative_carrier + positive_carrier) / mass[4]);
            assert!(
                ((negative_marker + positive_marker) - expected_marker).abs() <= marker[4] * 2e-7,
                "axis {axis}: marker budget"
            );
        }
        if intermediate_sweep {
            assert!(
                negative_carrier > mass[4] - positive_carrier,
                "axis {axis}: measured fixture must cross the old subtraction guard"
            );
        }
        for face in [1, 2] {
            let offset = face * 5;
            for phase in 0..2 {
                let phase_mass = flux[offset + 1 + phase].abs();
                let phase_marker = flux[offset + 3 + phase].abs();
                assert!(phase_mass.is_finite() && phase_marker.is_finite());
                assert!(
                    phase_mass > 0.0 || phase_marker == 0.0,
                    "axis {axis}: face {face} phase {phase} orphan marker"
                );
            }
            let volume_from_phases = flux[offset + 1].abs() / 1000.0 + flux[offset + 2].abs() / 1.2;
            assert!(
                (volume_from_phases - flux[offset].abs()).abs() <= flux[offset].abs() * 1e-5,
                "axis {axis}: face {face} volume changed"
            );
        }
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("two-outlet gather fixture"),
            });
        run_pass(&mut encoder, gather_pipeline, &gather_bindings, 3);
        gpu.queue.submit([encoder.finish()]);
        let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
        let expected_status = if intermediate_sweep && axis == 1 {
            STATUS_VOLUME_CLOSURE
        } else {
            0
        };
        assert_eq!(
            u32::from_le_bytes(status.try_into().unwrap()),
            expected_status,
            "axis {axis}: gathered inventory"
        );
        let result_mass = read_f32(&gpu.device, &gpu.queue, output_mass);
        let result_marker = read_f32(&gpu.device, &gpu.queue, output_marker);
        if exhausts_carrier {
            assert_eq!(result_mass[4], 0.0, "axis {axis}: exhausted carrier");
            assert_eq!(result_marker[4], 0.0, "axis {axis}: exhausted marker");
        } else {
            assert!(result_mass[4] > 0.0 && result_marker[4] > 0.0);
        }
        assert!(
            result_mass
                .iter()
                .chain(&result_marker)
                .all(|value| value.is_finite() && *value >= 0.0)
        );
        for (phase_cell, (phase_mass, phase_marker)) in
            result_mass.iter().zip(&result_marker).enumerate()
        {
            assert!(
                *phase_mass > 0.0 || *phase_marker == 0.0,
                "axis {axis}: gathered phase slot {phase_cell} orphan marker"
            );
        }
        for (slot, (before, after)) in [
            (&mass[..3], &result_mass[..3]),
            (&mass[3..], &result_mass[3..]),
            (&marker[..3], &result_marker[..3]),
            (&marker[3..], &result_marker[3..]),
        ]
        .into_iter()
        .enumerate()
        {
            let before: f64 = before.iter().map(|value| f64::from(*value)).sum();
            let after: f64 = after.iter().map(|value| f64::from(*value)).sum();
            assert!(
                (after - before).abs() <= before * 2e-7,
                "axis {axis}: inventory slot {slot} drifted: {before:e} to {after:e}"
            );
        }
    }
}
