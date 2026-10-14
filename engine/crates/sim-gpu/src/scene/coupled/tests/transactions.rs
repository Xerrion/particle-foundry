use super::*;

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn closed_scene_accepts_one_complete_zero_force_tick() {
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
        let before_time = scene.scene.accepted_time_s;
        let before_index = scene.scene.committed_index;
        let outcome = scene.advance_outer_tick(4).await.unwrap();
        match outcome {
            GpuTickOutcome::Complete {
                accepted_substeps,
                last_pressure,
                encoded_time_error_s,
                ..
            } => {
                assert_eq!(accepted_substeps, 1);
                assert!(last_pressure.scaled_residual <= 1e-5);
                assert!(last_pressure.scaled_divergence <= 1e-5);
                assert!(encoded_time_error_s.abs() < 1e-9);
            }
            GpuTickOutcome::Paused { .. } => panic!("zero flow should complete one tick"),
        }
        assert_eq!(scene.scene.committed_index, 1 - before_index);
        assert_eq!(scene.scene.tick, 8);
        assert!((scene.scene.accepted_time_s - before_time - OUTER_DT_S).abs() < 1e-15);
    });
}

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn numerical_retry_counts_attempts_and_respects_accepted_substep_budget() {
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
        let before = scene.progress();
        let committed_before = read_fixture_committed_state(&scene.scene).await.unwrap();
        scene.test_numerical_rejections = 1;
        let GpuTickOutcome::Paused {
            accepted_substeps,
            attempted_candidates,
            refinement_retries,
            remaining_s,
            readback_bytes,
            rejected_readback_bytes,
        } = scene.advance_outer_tick(1).await.unwrap()
        else {
            panic!("one accepted refined step must pause with remaining time");
        };
        assert_eq!(accepted_substeps, 1);
        assert_eq!(attempted_candidates, 2);
        assert_eq!(refinement_retries, 1);
        assert!(rejected_readback_bytes >= STAGE_STATUS_BYTES + 32);
        assert_eq!(readback_bytes, rejected_readback_bytes + 8);
        assert_eq!(scene.scene.tick, before.tick);
        assert_eq!(scene.scene.state_revision, before.state_revision + 1);
        assert_eq!(scene.scene.committed_index, 1 - before.committed_generation);
        assert!(remaining_s >= OUTER_DT_S * 0.5);
        assert!(
            (remaining_s + scene.scene.accepted_time_s - before.accepted_time_s - OUTER_DT_S).abs()
                < 1e-15
        );
        assert_eq!(
            read_fixture_committed_state(&scene.scene).await.unwrap(),
            committed_before
        );
        let GpuTickOutcome::Complete {
            accepted_substeps,
            attempted_candidates,
            refinement_retries,
            rejected_readback_bytes,
            encoded_time_error_s,
            ..
        } = scene.advance_outer_tick(1).await.unwrap()
        else {
            panic!("the remaining zero-force step must complete the outer tick");
        };
        assert_eq!(accepted_substeps, 1);
        assert_eq!(attempted_candidates, 1);
        assert_eq!(refinement_retries, 0);
        assert_eq!(rejected_readback_bytes, 0);
        assert_eq!(scene.scene.tick, before.tick + 1);
        assert_eq!(scene.remaining_outer_s, 0.0);
        assert!((scene.scene.accepted_time_s - before.accepted_time_s - OUTER_DT_S).abs() < 1e-15);
        assert!(encoded_time_error_s.abs() < 1e-9);
    });
}

#[test]
#[ignore = "requires a native GPU compute adapter"]
fn numerical_refinement_exhaustion_preserves_committed_fields_and_time() {
    block_on(async {
        let context = GpuContext::new().await.unwrap();
        let mut scene = GpuCoupledScene::new(
            &context,
            uniform_air_seed(),
            GpuScenePhysics {
                cell_dynamic_viscosity_pa_s: None,
                gravity_m_s2: 0.0,
                pressure: PressureConfig::default(),
            },
        )
        .unwrap();
        let before = scene.progress();
        let encoded_before = scene.encoded_time_s;
        let committed_before = read_fixture_committed_state(&scene.scene).await.unwrap();
        scene.test_numerical_rejections = MAX_CANDIDATE_REFINEMENTS + 1;
        let error = scene.advance_outer_tick(1).await.unwrap_err();
        assert!(
            error.contains("refinement exhausted after 5 attempts"),
            "{error}"
        );
        assert_eq!(scene.test_numerical_rejections, 0);
        assert_eq!(scene.progress(), before);
        assert_eq!(scene.encoded_time_s, encoded_before);
        assert_eq!(
            read_fixture_committed_state(&scene.scene).await.unwrap(),
            committed_before
        );
        assert!(matches!(
            scene.advance_outer_tick(1).await.unwrap(),
            GpuTickOutcome::Complete {
                accepted_substeps: 1,
                attempted_candidates: 1,
                refinement_retries: 0,
                ..
            }
        ));
    });
}
