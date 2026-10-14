use super::*;

impl GpuCoupledScene {
    /// Accepts up to `max_substeps` detached candidates. Each candidate permits
    /// at most four smaller retries for numerical rejection. A budget pause
    /// retains accepted substeps and the remaining portion of the tick.
    pub(crate) async fn advance_outer_tick(
        &mut self,
        max_substeps: u32,
    ) -> Result<GpuTickOutcome, String> {
        if max_substeps == 0 {
            return Err("GPU outer tick needs a positive substep budget".into());
        }
        if self.scene.tick == u64::MAX {
            return Err("GPU scene tick counter exhausted".into());
        }
        let mut remaining_outer_s = if self.remaining_outer_s == 0.0 {
            OUTER_DT_S
        } else {
            self.remaining_outer_s
        };
        let mut readback_bytes = 0_u64;
        let mut rejected_readback_bytes = 0_u64;
        let mut attempted_candidates = 0_u32;
        let mut refinement_retries = 0_u32;
        for accepted_substeps in 0..max_substeps {
            let measurement = self
                .cfl
                .measure(
                    &self.scene.context.device,
                    &self.scene.context.queue,
                    &self.scene.committed().u_velocity_m_s,
                    &self.scene.committed().v_velocity_m_s,
                )
                .await?;
            let minimum_density = conservative_minimum_density(self.scene.phase_density_kg_m3);
            let max_nu = if self.max_viscosity_pa_s == 0.0 {
                0.0
            } else {
                f64::from(self.max_viscosity_pa_s) / f64::from(minimum_density)
            };
            let mut selected = measurement.select_duration(remaining_outer_s, max_nu)?;
            let (mut nominal_dt_s, mut encoded_dt_s) = select_clock_durations(
                remaining_outer_s,
                selected,
                self.scene.accepted_time_s - self.encoded_time_s,
            )?;
            let mut candidate_refinements = 0_u32;
            let (pressure, next_remaining_s, next_encoded_time) = loop {
                let next_remaining_s = remaining_outer_s - nominal_dt_s;
                if next_remaining_s < 0.0 {
                    return Err("GPU scene selected more than remaining outer time".into());
                }
                let next_encoded_time = self.encoded_time_s + f64::from(encoded_dt_s);
                if !next_encoded_time.is_finite() || next_encoded_time <= self.encoded_time_s {
                    return Err("GPU encoded time cannot advance".into());
                }
                attempted_candidates = attempted_candidates
                    .checked_add(1)
                    .ok_or("GPU candidate attempt counter exhausted")?;
                let mut pressure_config = self.pressure_config;
                pressure_config.dt_s = encoded_dt_s;
                pressure_config.gravity_m_s2 = self.gravity_m_s2;
                match self
                    .advance_candidate(
                        encoded_dt_s,
                        nominal_dt_s,
                        self.gravity_m_s2,
                        pressure_config,
                    )
                    .await
                {
                    Ok(pressure) => break (pressure, next_remaining_s, next_encoded_time),
                    Err(CandidateRejection::Terminal(message)) => return Err(message),
                    Err(CandidateRejection::Numerical {
                        message,
                        readback_bytes: rejected_bytes,
                    }) => {
                        rejected_readback_bytes += rejected_bytes;
                        if candidate_refinements == MAX_CANDIDATE_REFINEMENTS {
                            return Err(format!(
                                "GPU candidate refinement exhausted after {} attempts at dt {encoded_dt_s}: {message}",
                                candidate_refinements + 1,
                            ));
                        }
                        selected = refined_selection(selected, nominal_dt_s)?;
                        (nominal_dt_s, encoded_dt_s) = select_clock_durations(
                            remaining_outer_s,
                            selected,
                            self.scene.accepted_time_s - self.encoded_time_s,
                        )?;
                        candidate_refinements += 1;
                        refinement_retries = refinement_retries
                            .checked_add(1)
                            .ok_or("GPU refinement counter exhausted")?;
                    }
                }
            };
            // Each accepted candidate maps an 8-byte CFL result, a 20-byte
            // stage status and the pressure solver's counted 32-byte records.
            readback_bytes += 8 + STAGE_STATUS_BYTES + pressure.readback_bytes;
            self.encoded_time_s = next_encoded_time;
            remaining_outer_s = next_remaining_s;
            self.remaining_outer_s = remaining_outer_s;
            if remaining_outer_s == 0.0 {
                self.scene.tick += 1;
                return Ok(GpuTickOutcome::Complete {
                    accepted_substeps: accepted_substeps + 1,
                    attempted_candidates,
                    refinement_retries,
                    last_pressure: pressure,
                    readback_bytes,
                    rejected_readback_bytes,
                    encoded_time_error_s: self.scene.accepted_time_s - self.encoded_time_s,
                });
            }
        }
        Ok(GpuTickOutcome::Paused {
            accepted_substeps: max_substeps,
            attempted_candidates,
            refinement_retries,
            remaining_s: remaining_outer_s,
            readback_bytes,
            rejected_readback_bytes,
        })
    }

    /// Advances one f32 substep. Any failure preserves the committed generation
    /// and accepted time for this substep, including pressure nonconvergence.
    pub(super) async fn advance_candidate(
        &mut self,
        dt_s: f32,
        nominal_dt_s: f64,
        gravity_m_s2: f32,
        pressure_config: PressureConfig,
    ) -> Result<PressureDiagnostics, CandidateRejection> {
        let next_time = self.scene.accepted_time_s + nominal_dt_s;
        if !next_time.is_finite() || next_time <= self.scene.accepted_time_s {
            return Err("GPU accepted time cannot advance".into());
        }
        let next_revision = self
            .scene
            .state_revision
            .checked_add(1)
            .ok_or("GPU scene state revision exhausted")?;
        let device = &self.scene.context.device;
        let queue = &self.scene.context.queue;
        let committed = self.scene.committed();
        let candidate = self.scene.candidate();
        let CandidateBuffers {
            transport,
            momentum,
            density,
            viscosity,
            gravity,
        } = &self.candidates;
        // This staging buffer is unique to the candidate. Dropping an async
        // step during map_async cannot poison a later candidate's readback.
        let stage_status_staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene stage status staging"),
            size: STAGE_STATUS_BYTES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("sealed GPU candidate stages"),
        });
        self.transport.encode(
            device,
            &mut encoder,
            &GpuTransportInput {
                mass_kg: &committed.mass_kg,
                marker: &committed.marker,
                headers: &self.scene.headers,
                u_velocity_m_s: &committed.u_velocity_m_s,
                v_velocity_m_s: &committed.v_velocity_m_s,
                u_aperture: &self.scene.u_aperture,
                v_aperture: &self.scene.v_aperture,
            },
            transport,
            GpuTransportStep {
                dt_s,
                liquid_density_kg_m3: self.scene.phase_density_kg_m3[0],
                carrier_density_kg_m3: self.scene.phase_density_kg_m3[1],
            },
        )?;
        self.momentum.encode(
            device,
            &mut encoder,
            &GpuMomentumInput {
                source_mass_kg: &committed.mass_kg,
                after_x_mass_kg: &transport.after_x_mass_kg,
                final_mass_kg: &transport.mass_kg,
                source_u_velocity_m_s: &committed.u_velocity_m_s,
                source_v_velocity_m_s: &committed.v_velocity_m_s,
                u_aperture: &self.scene.u_aperture,
                v_aperture: &self.scene.v_aperture,
                u_flux: &transport.u_flux,
                v_flux: &transport.v_flux,
            },
            momentum,
        )?;
        self.density.encode(
            device,
            &mut encoder,
            &transport.mass_kg,
            &self.scene.headers,
            density,
            self.scene.phase_density_kg_m3,
        )?;
        let (after_shear_u, after_shear_v) = if let (Some(stage), Some(field), Some(result)) =
            (&self.viscosity, &self.viscosity_field, viscosity)
        {
            stage.encode(
                device,
                &mut encoder,
                &GpuViscosityInput {
                    density_kg_m3: &density.density_kg_m3,
                    cell_dynamic_viscosity_pa_s: field,
                    u_velocity_m_s: &momentum.u_velocity_m_s,
                    v_velocity_m_s: &momentum.v_velocity_m_s,
                    u_aperture: &self.scene.u_aperture,
                    v_aperture: &self.scene.v_aperture,
                },
                result,
                GpuViscosityStep {
                    dt_s,
                    minimum_density_kg_m3: conservative_minimum_density(
                        self.scene.phase_density_kg_m3,
                    ),
                    maximum_dynamic_viscosity_pa_s: self.max_viscosity_pa_s,
                },
            )?;
            (&result.u_velocity_m_s, &result.v_velocity_m_s)
        } else {
            (&momentum.u_velocity_m_s, &momentum.v_velocity_m_s)
        };
        self.gravity.encode(
            device,
            &mut encoder,
            &GpuGravityInput {
                u_velocity_m_s: after_shear_u,
                v_velocity_m_s: after_shear_v,
                u_aperture: &self.scene.u_aperture,
                v_aperture: &self.scene.v_aperture,
            },
            gravity,
            GpuGravityStep { dt_s, gravity_m_s2 },
        )?;
        for (slot, status) in [
            Some(&transport.status),
            Some(&momentum.status),
            Some(&density.status),
            viscosity.as_ref().map(|value| &value.status),
            Some(&gravity.status),
        ]
        .into_iter()
        .enumerate()
        {
            let offset = (slot * size_of::<u32>()) as u64;
            if let Some(status) = status {
                encoder.copy_buffer_to_buffer(status, 0, &stage_status_staging, offset, 4);
            } else {
                encoder.clear_buffer(&stage_status_staging, offset, Some(4));
            }
        }
        queue.submit([encoder.finish()]);
        #[cfg(test)]
        if self.test_fault == Some(TestFaultPoint::SubmittedStages) {
            std::future::pending::<()>().await;
        }
        let statuses = read_stage_statuses(device, &stage_status_staging).await?;
        for (slot, (name, status)) in ["transport", "momentum", "density", "viscosity", "gravity"]
            .into_iter()
            .zip(statuses)
            .enumerate()
        {
            if status != 0 {
                return Err(stage_rejection(slot, name, status));
            }
            #[cfg(test)]
            if self.test_fault == Some(TestFaultPoint::Stage(slot)) {
                return Err(format!("injected GPU candidate failure after {name}").into());
            }
            #[cfg(not(test))]
            let _ = slot;
        }
        let pressure = self
            .pressure
            .project_closed(
                device,
                queue,
                PressureFields {
                    density: &density.density_kg_m3,
                    root_cell: &self.scene.component_root,
                    aperture_u: &self.scene.u_aperture,
                    aperture_v: &self.scene.v_aperture,
                    predictor_u: &gravity.u_velocity_m_s,
                    predictor_v: &gravity.v_velocity_m_s,
                    corrected_u: &candidate.u_velocity_m_s,
                    corrected_v: &candidate.v_velocity_m_s,
                },
                pressure_config,
            )
            .await
            .map_err(|error: PressureError| {
                // A zero solve budget is an explicit failure fixture. Do not
                // turn it into success by reducing the pressure forcing.
                if error.is_retryable() && pressure_config.max_iterations != 0 {
                    CandidateRejection::Numerical {
                        message: error.to_string(),
                        readback_bytes: STAGE_STATUS_BYTES + self.pressure.last_readback_bytes(),
                    }
                } else {
                    CandidateRejection::Terminal(error.to_string())
                }
            })?;
        #[cfg(test)]
        if self.test_fault == Some(TestFaultPoint::Pressure) {
            return Err("injected GPU candidate failure after pressure".into());
        }
        #[cfg(test)]
        if self.test_numerical_rejections > 0 {
            self.test_numerical_rejections -= 1;
            return Err(CandidateRejection::Numerical {
                message: "test numerical gate rejected candidate".into(),
                readback_bytes: STAGE_STATUS_BYTES + pressure.readback_bytes,
            });
        }
        #[cfg(test)]
        if self.test_fault == Some(TestFaultPoint::BeforeCommit) {
            std::future::pending::<()>().await;
        }
        let mut commit = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("accepted GPU inventory copy"),
        });
        for (source, target) in [
            (&transport.mass_kg, &candidate.mass_kg),
            (&transport.marker, &candidate.marker),
            (&density.density_kg_m3, &candidate.density_kg_m3),
        ] {
            commit.copy_buffer_to_buffer(source, 0, target, 0, target.size());
        }
        self.pressure.encode_accepted_guess(&mut commit);
        queue.submit([commit.finish()]);
        self.scene.committed_index = 1 - self.scene.committed_index;
        self.scene.accepted_time_s = next_time;
        self.scene.state_revision = next_revision;
        Ok(pressure)
    }
}
