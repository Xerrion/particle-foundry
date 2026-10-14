use super::*;

impl GpuCoupledScene {
    /// Returns this scene's grid for a viewport bound to its committed state.
    pub(crate) fn grid(&self) -> Grid {
        self.scene.grid
    }

    /// Reports only committed scene progress. An in-flight candidate does not
    /// alter these values until its pressure and stage gates succeed.
    pub(crate) fn progress(&self) -> GpuSceneProgress {
        GpuSceneProgress {
            epoch: self.scene.epoch,
            tick: self.scene.tick,
            accepted_time_s: self.scene.accepted_time_s,
            remaining_outer_s: self.remaining_outer_s,
            committed_generation: self.scene.committed_index,
            state_revision: self.scene.state_revision,
        }
    }

    fn probe_stamp(&self) -> ProbeStamp {
        ProbeStamp {
            epoch: self.scene.epoch,
            tick: self.scene.tick,
            accepted_time_s: self.scene.accepted_time_s,
            committed_generation: self.scene.committed_index,
            state_revision: self.scene.state_revision,
        }
    }

    fn checkpoint_stamp(&self) -> CheckpointStamp {
        CheckpointStamp {
            epoch: self.scene.epoch,
            tick: self.scene.tick,
            accepted_time_s: self.scene.accepted_time_s,
            remaining_outer_s: self.remaining_outer_s,
            generation: self.scene.committed_index as u32,
            state_revision: self.scene.state_revision,
        }
    }

    /// Reads one committed cell and rejects a result made stale by a later edit.
    pub(crate) async fn probe_cell(&self, x: u32, y: u32) -> Result<ProbeSample, String> {
        let context = &self.scene.context;
        let pending = self.probe.submit(
            &context.device,
            &context.queue,
            ProbeSource {
                grid: self.scene.grid,
                headers: &self.scene.headers,
                committed_mass_kg: &self.scene.committed().mass_kg,
                stamp: self.probe_stamp(),
            },
            [x, y],
        )?;
        let sample = pending.resolve(&context.device).await?;
        if !sample.is_current(self.probe_stamp()) {
            return Err("GPU probe result is stale".into());
        }
        Ok(sample)
    }

    /// Captures one detached generation only when explicitly requested.
    /// Versioned recovery and load remain separate E13 work.
    pub(crate) async fn checkpoint_prototype(&self) -> Result<Vec<u8>, String> {
        let context = &self.scene.context;
        let committed = self.scene.committed();
        let capture = checkpoint::capture(
            &context.device,
            &context.queue,
            CheckpointSource {
                grid: self.scene.grid,
                stamp: self.checkpoint_stamp(),
                fields: [
                    &committed.mass_kg,
                    &committed.marker,
                    &committed.density_kg_m3,
                    &committed.u_velocity_m_s,
                    &committed.v_velocity_m_s,
                ],
            },
        )
        .await?;
        if !capture.is_current(self.checkpoint_stamp()) {
            return Err("GPU checkpoint capture is stale".into());
        }
        Ok(capture.into_prototype_bytes())
    }

    /// Applies a bounded control edit to detached resident state at zero model time.
    pub(crate) fn paint(
        &mut self,
        center_x: u32,
        center_y: u32,
        radius: u32,
        material: GpuPaintMaterial,
    ) -> Result<(), String> {
        self.paint_stage
            .apply(&mut self.scene, center_x, center_y, radius, material)
    }

    /// Draws the selected committed generation on the scene's own device.
    /// A failed candidate never reaches this renderer. No simulation field is
    /// mapped or copied to the host for a normal frame.
    pub(crate) fn render_committed(
        &self,
        renderer: &SceneRenderer,
        target: &wgpu::TextureView,
        target_size_px: [u32; 2],
        viewport: RenderViewport,
    ) -> Result<(), String> {
        let context = &self.scene.context;
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("committed scene display"),
            });
        renderer.encode(
            &context.device,
            &mut encoder,
            target,
            target_size_px,
            viewport,
            RenderScene {
                grid: self.scene.grid,
                phase_density_kg_m3: self.scene.phase_density_kg_m3,
                headers: &self.scene.headers,
                committed_mass_kg: &self.scene.committed().mass_kg,
            },
        )?;
        context.queue.submit([encoder.finish()]);
        Ok(())
    }

    /// Preflights and constructs one private GPU scene with static wall geometry.
    pub(crate) fn new(
        context: &GpuContext,
        seed: GpuSceneSeed,
        physics: GpuScenePhysics,
    ) -> Result<Self, String> {
        let GpuScenePhysics {
            cell_dynamic_viscosity_pa_s,
            gravity_m_s2,
            pressure,
        } = physics;
        if !gravity_m_s2.is_finite() {
            return Err("GPU scene gravity must be finite".into());
        }
        if !pressure.scaled_residual_tolerance.is_finite()
            || pressure.scaled_residual_tolerance <= 0.0
            || !pressure.scaled_divergence_tolerance.is_finite()
            || pressure.scaled_divergence_tolerance <= 0.0
        {
            return Err("GPU scene pressure tolerances must be positive and finite".into());
        }
        if let Some(viscosity) = &cell_dynamic_viscosity_pa_s
            && (viscosity.len() != seed.grid.cells()
                || viscosity
                    .iter()
                    .any(|value| !value.is_finite() || *value <= 0.0))
        {
            return Err("GPU scene viscosity array is invalid".into());
        }
        let grid = seed.grid;
        let scene = GpuScene::new(context, seed)?;
        let paint_stage = GpuPaintStage::new(&context.device, grid)?;
        let cfl = GpuCflStage::new(&context.device, grid)?;
        let transport = GpuTransportStage::new(&context.device, grid)?;
        let momentum = GpuMomentumStage::new(&context.device, grid)?;
        let density = GpuDensityDeriver::new(&context.device, grid)?;
        let (viscosity, viscosity_field, max_viscosity_pa_s) =
            if let Some(values) = cell_dynamic_viscosity_pa_s {
                let maximum = values.iter().copied().fold(0.0_f32, f32::max);
                let field = context
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("scene fixed dynamic viscosity"),
                        contents: &super::super::f32_bytes(&values),
                        usage: wgpu::BufferUsages::STORAGE,
                    });
                (
                    Some(GpuViscosityStage::new(&context.device, grid)?),
                    Some(field),
                    maximum,
                )
            } else {
                (None, None, 0.0)
            };
        let gravity = GpuGravityStage::new(&context.device, grid)?;
        let pressure_stage = GpuPressureProjector::new(&context.device, grid)?;
        let probe = GpuCellProbe::new(&context.device)?;
        if STAGE_STATUS_BYTES > context.device.limits().max_buffer_size {
            return Err("GPU scene stage status staging exceeds adapter limit".into());
        }
        let candidates = CandidateBuffers::new(&context.device, grid, viscosity.is_some())?;
        let encoded_time_s = scene.accepted_time_s;
        Ok(Self {
            scene,
            candidates,
            paint_stage,
            cfl,
            transport,
            momentum,
            density,
            viscosity,
            viscosity_field,
            max_viscosity_pa_s,
            gravity_m_s2,
            gravity,
            pressure_config: pressure,
            pressure: pressure_stage,
            probe,
            remaining_outer_s: 0.0,
            encoded_time_s,
            #[cfg(test)]
            test_fault: None,
            #[cfg(test)]
            test_numerical_rejections: 0,
        })
    }
}
