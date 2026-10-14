use super::*;

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn pressure_rejection_preserves_committed_bytes_and_retry_matches_clean_run() {
    block_on(async {
        let context = GpuContext::new().await.unwrap();
        let make_scene = || {
            GpuCoupledScene::new(
                &context,
                mixed_vortex_seed(),
                GpuScenePhysics {
                    cell_dynamic_viscosity_pa_s: Some(vec![0.001; 4]),
                    gravity_m_s2: 0.0,
                    pressure: PressureConfig::default(),
                },
            )
            .unwrap()
        };
        let mut retried = make_scene();
        let mut clean = make_scene();
        assert!(matches!(
            retried.advance_outer_tick(8).await.unwrap(),
            GpuTickOutcome::Complete { .. }
        ));
        assert!(matches!(
            clean.advance_outer_tick(8).await.unwrap(),
            GpuTickOutcome::Complete { .. }
        ));
        assert_eq!(retried.scene.committed_index, 1);
        assert_eq!(
            read_fixture_committed_state(&retried.scene).await.unwrap(),
            read_fixture_committed_state(&clean.scene).await.unwrap()
        );
        let before = committed_metadata(&retried);
        let committed_before = read_fixture_committed_state(&retried.scene).await.unwrap();
        retried.gravity_m_s2 = 9.80665;
        retried.pressure_config.max_iterations = 0;
        let error = retried.advance_outer_tick(8).await.unwrap_err();
        assert!(
            error.contains("exhausted its bounded iterations"),
            "unexpected pressure error: {error}"
        );
        assert_eq!(committed_metadata(&retried), before);
        assert_eq!(
            read_fixture_committed_state(&retried.scene).await.unwrap(),
            committed_before
        );
        retried.gravity_m_s2 = 0.0;
        retried.pressure_config.max_iterations = PressureConfig::default().max_iterations;
        assert!(matches!(
            retried.advance_outer_tick(8).await.unwrap(),
            GpuTickOutcome::Complete { .. }
        ));
        assert!(matches!(
            clean.advance_outer_tick(8).await.unwrap(),
            GpuTickOutcome::Complete { .. }
        ));
        assert_eq!(committed_metadata(&retried), committed_metadata(&clean));
        assert_eq!(
            read_fixture_committed_state(&retried.scene).await.unwrap(),
            read_fixture_committed_state(&clean.scene).await.unwrap()
        );
    });
}

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn injected_faults_after_each_stage_and_pressure_preserve_committed_state() {
    block_on(async {
        let context = GpuContext::new().await.unwrap();
        let mut scene = GpuCoupledScene::new(
            &context,
            uniform_air_seed(),
            GpuScenePhysics {
                cell_dynamic_viscosity_pa_s: Some(vec![0.001; 4]),
                gravity_m_s2: 0.0,
                pressure: PressureConfig::default(),
            },
        )
        .unwrap();
        assert!(matches!(
            scene.advance_outer_tick(8).await.unwrap(),
            GpuTickOutcome::Complete { .. }
        ));
        assert_eq!(scene.scene.committed_index, 1);
        let before = committed_metadata(&scene);
        let committed_before = read_fixture_committed_state(&scene.scene).await.unwrap();
        for fault in [
            TestFaultPoint::Stage(0),
            TestFaultPoint::Stage(1),
            TestFaultPoint::Stage(2),
            TestFaultPoint::Stage(3),
            TestFaultPoint::Stage(4),
            TestFaultPoint::Pressure,
        ] {
            scene.test_fault = Some(fault);
            let error = scene.advance_outer_tick(8).await.unwrap_err();
            assert!(
                error.contains("injected GPU candidate failure"),
                "unexpected {fault:?} error: {error}"
            );
            assert_eq!(committed_metadata(&scene), before, "fault: {fault:?}");
            assert_eq!(
                read_fixture_committed_state(&scene.scene).await.unwrap(),
                committed_before,
                "fault: {fault:?}"
            );
        }
        scene.test_fault = None;
        assert!(matches!(
            scene.advance_outer_tick(8).await.unwrap(),
            GpuTickOutcome::Complete { .. }
        ));
    });
}
