use super::*;
use wgpu::util::DeviceExt;

impl GpuPressureProjector {
    /// Projects a sealed, zero-volume-source predictor into distinct candidate
    /// face buffers. The caller must publish them only after this returns `Ok`.
    /// Every iteration and reduction stays on the GPU. A 32-byte completion
    /// record is mapped per bounded batch. No full cell or face array is read back.
    pub(crate) async fn project_closed(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        fields: PressureFields<'_>,
        config: PressureConfig,
    ) -> Result<PressureDiagnostics, PressureError> {
        self.last_readback_bytes = 0;
        validate_config(config)?;
        self.check_fields(device, &fields)?;
        let mut params = Vec::with_capacity(PARAM_BYTES as usize);
        for word in [
            self.grid.width(),
            self.grid.height(),
            self.grid.cells() as u32,
            self.partial_count,
        ] {
            params.extend_from_slice(&word.to_le_bytes());
        }
        for word in [
            self.grid.cell_width_m() as f32,
            config.dt_s,
            config.scaled_residual_tolerance,
            config.scaled_divergence_tolerance,
            config.gravity_m_s2,
            if self.has_accepted_guess && config.max_iterations > 0 {
                1.0
            } else {
                0.0
            },
            0.0,
            0.0,
        ] {
            params.extend_from_slice(&word.to_le_bytes());
        }
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pressure parameters"),
            contents: &params,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        queue.write_buffer(&self.status, 0, &[0; STATUS_BYTES as usize]);

        let bindings = self.bindings(device, fields, &uniform, config);
        let init_pipeline = if bindings.hydrostatic_bind.is_some() {
            &self.pipelines.init_preconditioner_from_guess
        } else {
            &self.pipelines.init_preconditioner
        };

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("pressure PCG initialization"),
        });
        if self.has_accepted_guess && config.max_iterations > 0 {
            encoder.copy_buffer_to_buffer(
                &self.accepted_guess,
                0,
                &self.pressure,
                0,
                self.pressure.size(),
            );
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("pressure initialization stages"),
                timestamp_writes: None,
            });
            dispatch(
                &mut pass,
                &self.pipelines.rhs,
                &bindings.rhs_bind,
                self.partial_count,
            );
            if let Some(hydrostatic_bind) = &bindings.hydrostatic_bind {
                dispatch(
                    &mut pass,
                    &self.pipelines.hydrostatic_reference,
                    bindings
                        .hydrostatic_reference_bind
                        .as_ref()
                        .expect("gravity reference binding"),
                    self.grid.height().div_ceil(WORKGROUP_SIZE),
                );
                dispatch(
                    &mut pass,
                    &self.pipelines.hydrostatic,
                    hydrostatic_bind,
                    self.grid.width().div_ceil(WORKGROUP_SIZE),
                );
            }
            dispatch(
                &mut pass,
                init_pipeline,
                &bindings.init_preconditioner,
                self.partial_count,
            );
        }
        encoder.copy_buffer_to_buffer(&self.preconditioned, 0, &self.search, 0, self.search.size());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("pressure initial reduction"),
                timestamp_writes: None,
            });
            dispatch(
                &mut pass,
                &self.pipelines.reduce_initial,
                &bindings.reduce_initial,
                self.partial_count,
            );
            dispatch(
                &mut pass,
                &self.pipelines.finish_initial,
                &bindings.finish_initial,
                1,
            );
        }
        let staging = stage_status_readback(device, &mut encoder, &self.status);
        queue.submit([encoder.finish()]);
        self.last_readback_bytes += STATUS_BYTES;
        let mut status = read_status(device, staging).await?;
        let mut status_readbacks = 1_u64;
        if status.invalid != 0 {
            return Err(PressureError::Invalid(format!(
                "GPU pressure arithmetic rejected the initial fields: {status:?}"
            )));
        }

        while status.converged == 0 && status.iterations < config.max_iterations {
            let previous_iterations = status.iterations;
            let batch = (config.max_iterations - status.iterations).min(ITERATIONS_PER_BATCH);
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("bounded pressure PCG batch"),
            });
            {
                // wgpu tracks buffer dependencies per dispatch within a pass.
                // Keep the PCG sequence and map once per bounded batch.
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("bounded pressure PCG stages"),
                    timestamp_writes: None,
                });
                for _ in 0..batch {
                    dispatch(
                        &mut pass,
                        &self.pipelines.apply_search,
                        &bindings.apply_search,
                        self.partial_count,
                    );
                    dispatch(
                        &mut pass,
                        &self.pipelines.reduce_denominator,
                        &bindings.reduce_denominator,
                        self.partial_count,
                    );
                    dispatch(
                        &mut pass,
                        &self.pipelines.finish_alpha,
                        &bindings.finish_alpha,
                        1,
                    );
                    dispatch(
                        &mut pass,
                        &self.pipelines.update_solution,
                        &bindings.update_solution,
                        self.partial_count,
                    );
                    dispatch(
                        &mut pass,
                        &self.pipelines.update_preconditioner,
                        &bindings.update_preconditioner,
                        self.partial_count,
                    );
                    dispatch(
                        &mut pass,
                        &self.pipelines.reduce_cached,
                        &bindings.reduce_cached,
                        self.partial_count,
                    );
                    dispatch(
                        &mut pass,
                        &self.pipelines.finish_beta,
                        &bindings.finish_beta,
                        1,
                    );
                    dispatch(
                        &mut pass,
                        &self.pipelines.update_search,
                        &bindings.update_search,
                        self.partial_count,
                    );
                }
            }
            let staging = stage_status_readback(device, &mut encoder, &self.status);
            queue.submit([encoder.finish()]);
            self.last_readback_bytes += STATUS_BYTES;
            status = read_status(device, staging).await?;
            status_readbacks += 1;
            if status.invalid != 0 {
                return Err(PressureError::Invalid(format!(
                    "GPU pressure arithmetic failed: {status:?}"
                )));
            }
            if status.converged == 0 && status.iterations <= previous_iterations {
                return Err(PressureError::Invalid(format!(
                    "GPU pressure PCG made no bounded progress: {status:?}"
                )));
            }
        }
        if status.converged == 0 {
            // The half-tolerance stop reserves rounding headroom. At the fixed
            // iteration cap, a residual already inside the final gate can
            // still be corrected and measured. Neither gate is bypassed.
            let within_final_residual = status.scaled_residual <= config.scaled_residual_tolerance;
            if config.max_iterations == 0 || !within_final_residual {
                return Err(PressureError::Nonconvergence(format!(
                    "GPU pressure projection exhausted its bounded iterations: {status:?}"
                )));
            }
            // Status.converged is word six in pressure_finish.wgsl. This only
            // enables final validation; publication still requires its result.
            queue.write_buffer(&self.status, CONVERGED_WORD_OFFSET, &1_u32.to_le_bytes());
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("pressure correction and divergence gate"),
        });
        let u_groups = (self.grid.u_faces() as u32).div_ceil(WORKGROUP_SIZE);
        let v_groups = (self.grid.v_faces() as u32).div_ceil(WORKGROUP_SIZE);
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("pressure correction stages"),
                timestamp_writes: None,
            });
            // The hydrostatic seed uses the top of each open column as its pressure
            // reference. Mean-centering it magnifies f32 cancellation in thin air.
            if bindings.hydrostatic_bind.is_none() {
                dispatch(
                    &mut pass,
                    &self.pipelines.gauge_reset,
                    &bindings.gauge_reset,
                    self.partial_count,
                );
                dispatch(
                    &mut pass,
                    &self.pipelines.gauge_link,
                    &bindings.gauge_link,
                    self.partial_count,
                );
                dispatch(
                    &mut pass,
                    &self.pipelines.gauge_mean,
                    &bindings.gauge_mean,
                    self.partial_count,
                );
                dispatch(
                    &mut pass,
                    &self.pipelines.gauge_center,
                    &bindings.gauge_center,
                    self.partial_count,
                );
            }
            dispatch(
                &mut pass,
                &self.pipelines.reduce_updated,
                &bindings.reduce_updated,
                self.partial_count,
            );
            dispatch(
                &mut pass,
                &self.pipelines.finish_final_residual,
                &bindings.finish_final_residual,
                1,
            );
            dispatch(
                &mut pass,
                &self.pipelines.correct_u,
                &bindings.correct_u,
                u_groups,
            );
            dispatch(
                &mut pass,
                &self.pipelines.correct_v,
                &bindings.correct_v,
                v_groups,
            );
            dispatch(
                &mut pass,
                &self.pipelines.divergence,
                &bindings.divergence,
                self.partial_count,
            );
            dispatch(
                &mut pass,
                &self.pipelines.finish_completion,
                &bindings.finish_completion,
                1,
            );
        }
        let staging = stage_status_readback(device, &mut encoder, &self.status);
        queue.submit([encoder.finish()]);
        self.last_readback_bytes += STATUS_BYTES;
        status = read_status(device, staging).await?;
        status_readbacks += 1;
        let diagnostics = PressureDiagnostics {
            iterations: status.iterations,
            scaled_residual: status.scaled_residual,
            scaled_divergence: status.scaled_divergence,
            readback_bytes: status_readbacks * STATUS_BYTES,
        };
        if status.invalid != 0 || status.converged == 0 {
            let message = format!("GPU pressure projection failed its final gate: {status:?}");
            return Err(if status.invalid != 0 {
                PressureError::Invalid(message)
            } else {
                PressureError::Nonconvergence(message)
            });
        }
        Ok(diagnostics)
    }
}
