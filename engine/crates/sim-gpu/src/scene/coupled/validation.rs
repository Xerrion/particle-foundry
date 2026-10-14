use super::*;

pub(super) fn e07_uniform_air_seed() -> GpuSceneSeed {
    let grid = Grid::new(2.0, 2.0).expect("fixed E07 fixture grid");
    let volume = grid.cell_volume_m3();
    GpuSceneSeed {
        grid,
        phase_density_kg_m3: [1000.0, 1.0],
        mass_kg: [vec![0.0; grid.cells()], vec![volume; grid.cells()]].concat(),
        marker: vec![0.0; grid.cells() * 2],
        energy_j: vec![0.0; grid.cells()],
        fixed_wall: vec![0; grid.cells()],
        u_velocity_m_s: vec![0.0; grid.u_faces()],
        v_velocity_m_s: vec![0.0; grid.v_faces()],
        u_aperture: vec![0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
        v_aperture: vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0],
        epoch: 2,
        tick: 7,
        accepted_time_s: 7.0 / 60.0,
    }
}

pub(super) fn e07_mixed_vortex_seed() -> GpuSceneSeed {
    let mut seed = e07_uniform_air_seed();
    seed.phase_density_kg_m3[1] = 1.2;
    let volume = seed.grid.cell_volume_m3();
    let fractions = [1.0, 0.0, 0.25, 0.75];
    seed.mass_kg = fractions
        .iter()
        .map(|&alpha| alpha * 1000.0 * volume)
        .chain(fractions.iter().map(|&alpha| (1.0 - alpha) * 1.2 * volume))
        .collect();
    seed.marker = fractions
        .iter()
        .map(|&alpha| 0.4 * alpha)
        .chain(fractions.iter().map(|&alpha| 0.8 * (1.0 - alpha)))
        .collect();
    seed.u_velocity_m_s[1] = 0.01;
    seed.u_velocity_m_s[4] = -0.01;
    seed.v_velocity_m_s[2] = -0.01;
    seed.v_velocity_m_s[3] = 0.01;
    seed
}

/// Runs one actual coupled GPU step and one rejected candidate on the same
/// browser or native device. Full committed-state downloads are limited to
/// this qualification fixture; normal steps read compact reductions only.
pub(crate) async fn validate_scene_fixture(
    context: &GpuContext,
) -> Result<GpuSceneValidationReport, String> {
    let mut working = GpuCoupledScene::new(
        context,
        e07_mixed_vortex_seed(),
        GpuScenePhysics {
            cell_dynamic_viscosity_pa_s: Some(vec![0.001; 4]),
            gravity_m_s2: 0.0,
            pressure: PressureConfig::default(),
        },
    )?;
    let GpuTickOutcome::Complete {
        accepted_substeps,
        last_pressure,
        encoded_time_error_s,
        ..
    } = working.advance_outer_tick(8).await?
    else {
        return Err("E07 browser fixture paused before completing one GPU tick".into());
    };
    // The accepted step switched generations. A rejected candidate now uses
    // the other generation, so this checks the actual committed GPU buffers.
    let before = (
        working.scene.epoch,
        working.scene.committed_index,
        working.scene.tick,
        working.scene.accepted_time_s,
        working.encoded_time_s,
        working.remaining_outer_s,
    );
    let committed_before = read_fixture_committed_state(&working.scene).await?;
    working.gravity_m_s2 = 9.80665;
    working.pressure_config.max_iterations = 0;
    let rejected = working
        .advance_outer_tick(8)
        .await
        .is_err_and(|error| error.contains("exhausted its bounded iterations"));
    let committed_after = read_fixture_committed_state(&working.scene).await?;
    let rejection_kept_committed_state = rejected
        && before
            == (
                working.scene.epoch,
                working.scene.committed_index,
                working.scene.tick,
                working.scene.accepted_time_s,
                working.encoded_time_s,
                working.remaining_outer_s,
            )
        && committed_before == committed_after;
    if !rejection_kept_committed_state {
        return Err("E07 rejected GPU candidate changed committed scene state".into());
    }
    Ok(GpuSceneValidationReport {
        backend: context.backend(),
        epoch: working.scene.epoch,
        tick: working.scene.tick,
        accepted_time_s: working.scene.accepted_time_s,
        accepted_substeps,
        scaled_residual: last_pressure.scaled_residual,
        scaled_divergence: last_pressure.scaled_divergence,
        encoded_time_error_s,
        rejection_kept_committed_state,
    })
}
