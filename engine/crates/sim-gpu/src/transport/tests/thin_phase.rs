use super::*;

#[test]
#[ignore = "requires a native GPU adapter"]
fn tangent_thin_phase_flux_matches_cpu_on_both_axes() {
    let gpu = block_on(crate::GpuContext::new()).expect("native GPU transport adapter");
    let grid = Grid::new(3.0, 3.0).unwrap();
    let cells = grid.cells();
    let volume = grid.cell_volume_m3();
    let center = grid.cell_index(1, 1).unwrap();
    let dt = 0.0025;
    for (axis, fractions) in [
        (0_u32, [0.0, 0.0, 0.0, 0.009, 0.01, 0.011, 1.0, 1.0, 1.0]),
        (1_u32, [0.0, 0.009, 1.0, 0.0, 0.01, 1.0, 0.0, 0.011, 1.0]),
    ] {
        for direction in [-1.0, 1.0] {
            let liquid = fractions.map(|alpha| alpha * volume * 1000.0);
            let carrier = fractions.map(|alpha| (1.0 - alpha) * volume * 1.2);
            let liquid_marker = fractions.map(|alpha| alpha * 2.5);
            let carrier_marker = fractions.map(|alpha| (1.0 - alpha) * 0.5);
            let initial = TransportInventory::new(
                grid,
                1000.0,
                1.2,
                liquid.to_vec(),
                carrier.to_vec(),
                liquid_marker.to_vec(),
                carrier_marker.to_vec(),
                vec![false; cells],
            )
            .unwrap();
            let mut velocity = FaceValues {
                u: vec![0.0; grid.u_faces()],
                v: vec![0.0; grid.v_faces()],
            };
            let mut aperture = FaceValues {
                u: vec![0.0; grid.u_faces()],
                v: vec![0.0; grid.v_faces()],
            };
            let speed = direction * 0.4;
            let tangent_face = if axis == 0 {
                // A closed circulation along the thin horizontal layer
                // returns through the pure liquid row below it.
                for x in [1, 2] {
                    velocity.u[grid.u_face_index(x, 1).unwrap()] = speed;
                    velocity.u[grid.u_face_index(x, 2).unwrap()] = -speed;
                }
                velocity.v[grid.v_face_index(0, 2).unwrap()] = -speed;
                velocity.v[grid.v_face_index(2, 2).unwrap()] = speed;
                grid.u_face_index(if direction > 0.0 { 2 } else { 1 }, 1)
                    .unwrap()
            } else {
                // Rotate the circulation. Its X return faces avoid the
                // center donor, retaining the original thin phase for Y.
                for y in [1, 2] {
                    velocity.v[grid.v_face_index(1, y).unwrap()] = speed;
                    velocity.v[grid.v_face_index(2, y).unwrap()] = -speed;
                }
                velocity.u[grid.u_face_index(2, 0).unwrap()] = -speed;
                velocity.u[grid.u_face_index(2, 2).unwrap()] = speed;
                grid.v_face_index(1, if direction > 0.0 { 2 } else { 1 })
                    .unwrap()
            };
            for (open, speed) in aperture
                .u
                .iter_mut()
                .zip(&velocity.u)
                .chain(aperture.v.iter_mut().zip(&velocity.v))
            {
                *open = if *speed != 0.0 { 1.0 } else { 0.0 };
            }
            let fields = PressureFields::new(
                grid,
                [Boundary::Closed; 4],
                fractions
                    .map(|alpha| alpha * 1000.0 + (1.0 - alpha) * 1.2)
                    .to_vec(),
                vec![0.0; cells],
                FaceValues {
                    u: velocity.u.clone(),
                    v: velocity.v.clone(),
                },
                FaceValues {
                    u: aperture.u.clone(),
                    v: aperture.v.clone(),
                },
            )
            .unwrap();
            let cpu = initial.candidate(&fields, &velocity, dt).unwrap();
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
            let headers = (0..cells)
                .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
                .collect::<Vec<_>>();
            let to_f32 =
                |values: &[f64]| values.iter().map(|&value| value as f32).collect::<Vec<_>>();
            let mass_buffer = buffer(&gpu.device, "thin tangent mass", &f32_bytes(&mass));
            let marker_buffer = buffer(&gpu.device, "thin tangent marker", &f32_bytes(&marker));
            let header_buffer = buffer(&gpu.device, "thin tangent headers", &headers);
            let u_buffer = buffer(
                &gpu.device,
                "thin tangent u",
                &f32_bytes(&to_f32(&velocity.u)),
            );
            let v_buffer = buffer(
                &gpu.device,
                "thin tangent v",
                &f32_bytes(&to_f32(&velocity.v)),
            );
            let u_aperture = buffer(
                &gpu.device,
                "thin tangent u aperture",
                &f32_bytes(&to_f32(&aperture.u)),
            );
            let v_aperture = buffer(
                &gpu.device,
                "thin tangent v aperture",
                &f32_bytes(&to_f32(&aperture.v)),
            );
            let input = GpuTransportInput {
                mass_kg: &mass_buffer,
                marker: &marker_buffer,
                headers: &header_buffer,
                u_velocity_m_s: &u_buffer,
                v_velocity_m_s: &v_buffer,
                u_aperture: &u_aperture,
                v_aperture: &v_aperture,
            };
            let stage = GpuTransportStage::new(&gpu.device, grid).unwrap();
            let candidate = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("thin tangent transport parity"),
                });
            stage
                .encode(
                    &gpu.device,
                    &mut encoder,
                    &input,
                    &candidate,
                    GpuTransportStep {
                        dt_s: dt as f32,
                        liquid_density_kg_m3: 1000.0,
                        carrier_density_kg_m3: 1.2,
                    },
                )
                .unwrap();
            gpu.queue.submit([encoder.finish()]);
            let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
            assert_eq!(
                u32::from_le_bytes(status.try_into().unwrap()),
                0,
                "axis {axis}, direction {direction}"
            );
            let flux_buffer = if axis == 0 {
                &candidate.u_flux
            } else {
                &candidate.v_flux
            };
            let flux = read_f32(&gpu.device, &gpu.queue, flux_buffer);
            let pick = |values: &FaceValues| {
                if axis == 0 {
                    values.u[tangent_face]
                } else {
                    values.v[tangent_face]
                }
            };
            let expected_flux = [
                pick(&cpu.fluxes.volume_m3),
                pick(&cpu.fluxes.liquid_mass_kg),
                pick(&cpu.fluxes.carrier_mass_kg),
                pick(&cpu.fluxes.liquid_marker),
                pick(&cpu.fluxes.carrier_marker),
            ];
            let actual_flux = &flux[tangent_face * 5..tangent_face * 5 + 5];
            for (slot, (&actual, &expected)) in actual_flux.iter().zip(&expected_flux).enumerate() {
                assert!(
                    (f64::from(actual) - expected).abs() <= expected.abs() * 1e-5,
                    "axis {axis}, direction {direction}, face slot {slot}: GPU {actual:e}, CPU {expected:e}"
                );
            }
            let moved_liquid_fraction = f64::from(actual_flux[1].abs()) / liquid[center];
            assert!(
                (moved_liquid_fraction - 0.1).abs() < 1e-5,
                "tangent flow drained the thin phase: moved fraction {moved_liquid_fraction}"
            );
            assert!(actual_flux[3].abs() < marker[center]);
            let phase_volume = actual_flux[1].abs() / 1000.0 + actual_flux[2].abs() / 1.2;
            assert!((phase_volume - actual_flux[0].abs()).abs() <= actual_flux[0].abs() * 1e-5);
            let actual_mass = read_f32(&gpu.device, &gpu.queue, &candidate.mass_kg);
            let actual_marker = read_f32(&gpu.device, &gpu.queue, &candidate.marker);
            for (slot, (before, after, expected, scale)) in [
                (
                    &mass[..cells],
                    &actual_mass[..cells],
                    cpu.inventory.liquid_mass_kg(),
                    1000.0 * volume,
                ),
                (
                    &mass[cells..],
                    &actual_mass[cells..],
                    cpu.inventory.carrier_mass_kg(),
                    1.2 * volume,
                ),
                (
                    &marker[..cells],
                    &actual_marker[..cells],
                    cpu.inventory.liquid_marker(),
                    2.5,
                ),
                (
                    &marker[cells..],
                    &actual_marker[cells..],
                    cpu.inventory.carrier_marker(),
                    0.5,
                ),
            ]
            .into_iter()
            .enumerate()
            {
                for (cell, (&actual, &expected)) in after.iter().zip(expected).enumerate() {
                    assert!(actual.is_finite() && actual >= 0.0);
                    assert!(
                        (f64::from(actual) - expected).abs() <= scale * 1e-5,
                        "axis {axis}, direction {direction}, inventory {slot}, cell {cell}: GPU {actual:e}, CPU {expected:e}"
                    );
                }
                let sum =
                    |values: &[f32]| values.iter().map(|&value| f64::from(value)).sum::<f64>();
                assert!((sum(after) - sum(before)).abs() <= sum(before) * 1e-5);
            }
            for cell in 0..cells {
                let filled_volume = actual_mass[cell] / 1000.0 + actual_mass[cells + cell] / 1.2;
                assert!((f64::from(filled_volume) - volume).abs() <= volume * 1e-5);
                for phase in 0..2 {
                    let slot = phase * cells + cell;
                    assert!(
                        actual_mass[slot] > 0.0 || actual_marker[slot] == 0.0,
                        "axis {axis}, direction {direction}, phase {phase}, cell {cell}: orphan marker"
                    );
                }
            }
        }
    }
    gpu.dispose();
}
