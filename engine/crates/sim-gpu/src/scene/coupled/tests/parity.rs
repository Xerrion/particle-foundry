use super::*;

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn mixed_vortex_matches_cpu_at_one_accepted_outer_time() {
    block_on(async {
        let reference_seed = mixed_vortex_seed();
        let reference = cpu_mixed_vortex_step(&reference_seed);
        let context = GpuContext::new().await.unwrap();
        let mut gpu = GpuCoupledScene::new(
            &context,
            mixed_vortex_seed(),
            GpuScenePhysics {
                cell_dynamic_viscosity_pa_s: Some(vec![0.001; 4]),
                gravity_m_s2: 0.0,
                pressure: PressureConfig::default(),
            },
        )
        .unwrap();
        let outcome = gpu.advance_outer_tick(8).await.unwrap();
        let GpuTickOutcome::Complete {
            last_pressure,
            accepted_substeps,
            ..
        } = outcome
        else {
            panic!("mixed vortex did not complete the outer tick");
        };
        assert_eq!(accepted_substeps, 1);
        assert!(last_pressure.scaled_divergence <= 1e-5);
        assert!((gpu.scene.accepted_time_s - (7.0 / 60.0 + OUTER_DT_S)).abs() < 1e-14);

        // Full-state downloads are test-only oracle comparisons. Normal
        // scene stepping reads only compact status and CFL reductions.
        let committed = gpu.scene.committed();
        let mass = read_f32(&context, &committed.mass_kg);
        let marker = read_f32(&context, &committed.marker);
        let u = read_f32(&context, &committed.u_velocity_m_s);
        let v = read_f32(&context, &committed.v_velocity_m_s);
        let cpu_inventory = reference.transport_inventory().unwrap();
        let cpu_fields = reference.pressure_fields().unwrap();
        let volume = reference_seed.grid.cell_volume_m3();
        let mut alpha_absolute_error = 0.0;
        for (cell, &gpu_mass) in mass.iter().take(reference_seed.grid.cells()).enumerate() {
            let gpu_alpha = f64::from(gpu_mass) / (1000.0 * volume);
            alpha_absolute_error += (gpu_alpha - cpu_inventory.alpha(cell).unwrap()).abs();
        }
        alpha_absolute_error /= reference_seed.grid.cells() as f64;
        assert!(
            alpha_absolute_error <= 1e-5,
            "alpha MAE {alpha_absolute_error}"
        );
        let mut squared_velocity_error = 0.0;
        let mut count = 0;
        for (gpu_faces, cpu_faces) in [
            (&u, &cpu_fields.velocity_m_s().u),
            (&v, &cpu_fields.velocity_m_s().v),
        ] {
            for (&gpu_speed, &cpu_speed) in gpu_faces.iter().zip(cpu_faces) {
                squared_velocity_error += (f64::from(gpu_speed) - cpu_speed).powi(2);
                count += 1;
            }
        }
        let scaled_velocity_rms = (squared_velocity_error / f64::from(count)).sqrt() / 0.01;
        assert!(
            scaled_velocity_rms <= 1e-4,
            "scaled velocity RMS {scaled_velocity_rms}"
        );
        for (gpu_values, cpu_values, scale) in [
            (&mass[..4], cpu_inventory.liquid_mass_kg(), 0.001),
            (&mass[4..], cpu_inventory.carrier_mass_kg(), 1e-6),
            (&marker[..4], cpu_inventory.liquid_marker(), 1.0),
            (&marker[4..], cpu_inventory.carrier_marker(), 1.0),
        ] {
            let gpu_total: f64 = gpu_values.iter().map(|&value| f64::from(value)).sum();
            let cpu_total: f64 = cpu_values.iter().sum();
            assert!(
                (gpu_total - cpu_total).abs() <= 1e-5 * scale,
                "GPU total {gpu_total} differs from CPU {cpu_total}"
            );
        }
    });
}

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn public_fixture_validation_reports_acceptance_and_rejection() {
    block_on(async {
        let context = GpuContext::new().await.unwrap();
        let report = context.validate_scene_fixture().await.unwrap();
        assert_eq!(report.epoch, 2);
        assert_eq!(report.tick, 8);
        assert_eq!(report.accepted_substeps, 1);
        assert!((report.accepted_time_s - 8.0 / 60.0).abs() < 1e-14);
        assert!(report.scaled_residual <= 1e-5);
        assert!(report.scaled_divergence <= 1e-5);
        assert!(report.encoded_time_error_s.abs() < 1e-9);
        assert!(report.rejection_kept_committed_state);
    });
}
