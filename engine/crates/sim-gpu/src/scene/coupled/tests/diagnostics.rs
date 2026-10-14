use super::*;

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn pressure_journal_matches_candidate_results_without_extra_readback_or_state_changes() {
    block_on(async {
        let context = GpuContext::new().await.unwrap();
        let make_scene = || {
            GpuCoupledScene::new(
                &context,
                uniform_air_seed(),
                GpuScenePhysics {
                    cell_dynamic_viscosity_pa_s: None,
                    gravity_m_s2: 0.0,
                    pressure: PressureConfig::default(),
                },
            )
            .unwrap()
        };
        let mut recorded = make_scene();
        let mut clean = make_scene();
        assert!(
            recorded
                .take_pressure_diagnostics_json()
                .contains("\"capacity\":0")
        );
        recorded.enable_pressure_diagnostics(256).unwrap();
        recorded.test_numerical_rejections = 1;
        clean.test_numerical_rejections = 1;
        for _ in 0..2 {
            let actual = recorded.advance_outer_tick(1).await.unwrap();
            let expected = clean.advance_outer_tick(1).await.unwrap();
            assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
            assert_eq!(recorded.progress(), clean.progress());
            assert_eq!(
                read_fixture_committed_state(&recorded.scene).await.unwrap(),
                read_fixture_committed_state(&clean.scene).await.unwrap()
            );
        }
        let journal = recorded.take_pressure_diagnostics_json();
        assert_eq!(journal.matches("\"type\":\"candidateBegin\"").count(), 3);
        assert_eq!(journal.matches("\"outcome\":\"accepted\"").count(), 2);
        assert_eq!(
            journal
                .matches("\"outcome\":\"numericalRejection\"")
                .count(),
            1
        );
        assert!(journal.contains("\"candidateId\":3,\"attempt\":1"));
        assert!(journal.contains("\"candidateId\":2,\"attempt\":2"));
        assert!(journal.contains("\"droppedEvents\":0"));

        let before = recorded.progress();
        recorded.test_fault = Some(TestFaultPoint::Stage(0));
        assert!(recorded.advance_outer_tick(1).await.is_err());
        assert_eq!(recorded.progress(), before);
        let before_pressure_rejection = recorded.take_pressure_diagnostics_json();
        assert!(before_pressure_rejection.contains("\"outcome\":\"terminalRejection\""));
        assert!(!before_pressure_rejection.contains("pressureStatus"));

        let mut rejected = GpuCoupledScene::new(
            &context,
            mixed_vortex_seed(),
            GpuScenePhysics {
                cell_dynamic_viscosity_pa_s: Some(vec![0.001; 4]),
                gravity_m_s2: 9.80665,
                pressure: PressureConfig {
                    max_iterations: 0,
                    ..PressureConfig::default()
                },
            },
        )
        .unwrap();
        rejected.enable_pressure_diagnostics(256).unwrap();
        let rejected_before = rejected.progress();
        assert!(rejected.advance_outer_tick(1).await.is_err());
        assert_eq!(rejected.progress(), rejected_before);
        let pressure_rejection = rejected.take_pressure_diagnostics_json();
        assert!(pressure_rejection.contains("\"phase\":\"initialization\""));
        assert!(pressure_rejection.contains("\"outcome\":\"terminalRejection\""));
        assert!(!pressure_rejection.contains("finalGate"));
        recorded.disable_pressure_diagnostics();
        assert!(
            recorded
                .take_pressure_diagnostics_json()
                .contains("\"capacity\":0")
        );
    });
}
