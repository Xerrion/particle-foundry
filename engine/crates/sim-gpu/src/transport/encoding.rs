use particle_sim::{CELL_WIDTH_M, REPRESENTED_DEPTH_M};
use wgpu::util::DeviceExt;

use super::bindings::{budget_group, closure_group, face_group, gather_group};
use super::buffers::check_buffer_sizes;
use super::{
    GpuTransportCandidate, GpuTransportInput, GpuTransportStage, GpuTransportStep,
    LOCAL_CLOSURE_ROUNDS, WORKGROUP_SIZE,
};

impl GpuTransportStage {
    /// Encodes an isolated candidate from projected velocities. The caller
    /// submits the encoder, then reads `candidate.status`. Zero permits later
    /// coupled-stage validation; nonzero rejects the complete substep.
    pub fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        input: &GpuTransportInput<'_>,
        candidate: &GpuTransportCandidate,
        step: GpuTransportStep,
    ) -> Result<(), String> {
        if !step.dt_s.is_finite() || step.dt_s <= 0.0 {
            return Err("transport dt must be finite and positive".into());
        }
        if !step.liquid_density_kg_m3.is_finite()
            || !step.carrier_density_kg_m3.is_finite()
            || step.liquid_density_kg_m3 <= 0.0
            || step.carrier_density_kg_m3 <= 0.0
        {
            return Err("transport densities must be finite and positive".into());
        }
        check_buffer_sizes(self.grid, input, candidate)?;
        let cell_volume = (CELL_WIDTH_M * CELL_WIDTH_M * REPRESENTED_DEPTH_M) as f32;
        let mut params = [0u8; 32];
        for (slot, value) in [
            self.grid.width(),
            self.grid.height(),
            self.grid.cells() as u32,
            0,
        ]
        .into_iter()
        .enumerate()
        {
            params[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (slot, value) in [
            step.dt_s,
            step.liquid_density_kg_m3,
            step.carrier_density_kg_m3,
            cell_volume,
        ]
        .into_iter()
        .enumerate()
        {
            params[16 + slot * 4..20 + slot * 4].copy_from_slice(&value.to_le_bytes());
        }
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("transport step parameters"),
            contents: &params,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let residual = closure_group(
            device,
            &self.residual,
            &uniform,
            &[
                input.mass_kg,
                input.headers,
                input.u_velocity_m_s,
                input.v_velocity_m_s,
                input.u_aperture,
                input.v_aperture,
                &candidate.base_residual,
                &candidate.status,
            ],
        );
        let pair_groups = [0_u32, 2, 1, 3].map(|variant| {
            let mut pair_params = params;
            pair_params[12..16].copy_from_slice(&variant.to_le_bytes());
            let pair_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("transport local pair parameters"),
                contents: &pair_params,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            closure_group(
                device,
                &self.pair,
                &pair_uniform,
                &[
                    input.headers,
                    input.u_aperture,
                    input.v_aperture,
                    &candidate.base_residual,
                    &candidate.closed_u_velocity_m_s,
                    &candidate.closed_v_velocity_m_s,
                    &candidate.status,
                ],
            )
        });
        let x_faces = face_group(
            device,
            &self.x_face,
            &uniform,
            input.mass_kg,
            input.marker,
            input.headers,
            &candidate.closed_u_velocity_m_s,
            input.u_aperture,
            &candidate.provisional_u_flux,
            &candidate.status,
        );
        let budget_groups = [
            (
                0_u32,
                &candidate.provisional_u_flux,
                &candidate.u_flux,
                input.mass_kg,
                input.marker,
            ),
            (
                1_u32,
                &candidate.provisional_v_flux,
                &candidate.v_flux,
                &candidate.after_x_mass_kg,
                &candidate.after_x_marker,
            ),
        ]
        .map(|(axis, provisional, corrected, mass, marker)| {
            let mut budget_params = params;
            budget_params[12..16].copy_from_slice(&axis.to_le_bytes());
            let budget_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("transport donor budget params"),
                contents: &budget_params,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            budget_group(
                device,
                &self.budget,
                &budget_uniform,
                mass,
                marker,
                provisional,
                corrected,
                &candidate.status,
            )
        });
        let x_cells = gather_group(
            device,
            &self.x_gather,
            &uniform,
            input.mass_kg,
            input.marker,
            input.headers,
            &candidate.u_flux,
            &candidate.after_x_mass_kg,
            &candidate.after_x_marker,
            &candidate.status,
        );
        let y_faces = face_group(
            device,
            &self.y_face,
            &uniform,
            &candidate.after_x_mass_kg,
            &candidate.after_x_marker,
            input.headers,
            &candidate.closed_v_velocity_m_s,
            input.v_aperture,
            &candidate.provisional_v_flux,
            &candidate.status,
        );
        let y_cells = gather_group(
            device,
            &self.y_gather,
            &uniform,
            &candidate.after_x_mass_kg,
            &candidate.after_x_marker,
            input.headers,
            &candidate.v_flux,
            &candidate.mass_kg,
            &candidate.marker,
            &candidate.status,
        );
        encoder.clear_buffer(&candidate.status, 0, None);
        encoder.copy_buffer_to_buffer(
            input.u_velocity_m_s,
            0,
            &candidate.closed_u_velocity_m_s,
            0,
            candidate.closed_u_velocity_m_s.size(),
        );
        encoder.copy_buffer_to_buffer(
            input.v_velocity_m_s,
            0,
            &candidate.closed_v_velocity_m_s,
            0,
            candidate.closed_v_velocity_m_s.size(),
        );
        // Compute dispatches have separate usage scopes. wgpu inserts the
        // barriers for dependent storage accesses within this shared pass.
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("transport stage"),
            timestamp_writes: None,
        });
        dispatch(&mut pass, &self.residual, &residual, self.grid.cells());
        pass.set_pipeline(&self.pair);
        for _ in 0..LOCAL_CLOSURE_ROUNDS {
            for (group, entries) in pair_groups.iter().zip([
                self.grid.u_faces(),
                self.grid.u_faces(),
                self.grid.v_faces(),
                self.grid.v_faces(),
            ]) {
                pass.set_bind_group(0, group, &[]);
                pass.dispatch_workgroups((entries as u32).div_ceil(WORKGROUP_SIZE), 1, 1);
            }
        }
        dispatch(&mut pass, &self.x_face, &x_faces, self.grid.u_faces());
        dispatch(
            &mut pass,
            &self.budget,
            &budget_groups[0],
            self.grid.u_faces(),
        );
        dispatch(&mut pass, &self.x_gather, &x_cells, self.grid.cells());
        dispatch(&mut pass, &self.y_face, &y_faces, self.grid.v_faces());
        dispatch(
            &mut pass,
            &self.budget,
            &budget_groups[1],
            self.grid.v_faces(),
        );
        dispatch(&mut pass, &self.y_gather, &y_cells, self.grid.cells());
        Ok(())
    }
}

fn dispatch(
    pass: &mut wgpu::ComputePass<'_>,
    pipeline: &wgpu::ComputePipeline,
    bindings: &wgpu::BindGroup,
    entries: usize,
) {
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bindings, &[]);
    pass.dispatch_workgroups((entries as u32).div_ceil(WORKGROUP_SIZE), 1, 1);
}
