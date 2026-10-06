use super::*;

pub(super) struct PressureBindings {
    pub(super) rhs_bind: wgpu::BindGroup,
    pub(super) hydrostatic_bind: Option<wgpu::BindGroup>,
    pub(super) hydrostatic_reference_bind: Option<wgpu::BindGroup>,
    pub(super) init_preconditioner: wgpu::BindGroup,
    pub(super) apply_search: wgpu::BindGroup,
    pub(super) update_solution: wgpu::BindGroup,
    pub(super) update_preconditioner: wgpu::BindGroup,
    pub(super) update_search: wgpu::BindGroup,
    pub(super) reduce_initial: wgpu::BindGroup,
    pub(super) reduce_denominator: wgpu::BindGroup,
    pub(super) reduce_updated: wgpu::BindGroup,
    pub(super) finish_initial: wgpu::BindGroup,
    pub(super) finish_alpha: wgpu::BindGroup,
    pub(super) finish_beta: wgpu::BindGroup,
    pub(super) finish_completion: wgpu::BindGroup,
    pub(super) correct_u: wgpu::BindGroup,
    pub(super) correct_v: wgpu::BindGroup,
    pub(super) divergence: wgpu::BindGroup,
    pub(super) gauge_reset: wgpu::BindGroup,
    pub(super) gauge_link: wgpu::BindGroup,
    pub(super) gauge_mean: wgpu::BindGroup,
    pub(super) gauge_center: wgpu::BindGroup,
    pub(super) finish_final_residual: wgpu::BindGroup,
}

impl GpuPressureProjector {
    pub(super) fn bindings(
        &self,
        device: &wgpu::Device,
        fields: PressureFields<'_>,
        uniform: &wgpu::Buffer,
        config: PressureConfig,
    ) -> PressureBindings {
        let rhs_bind = bind(
            device,
            &self.pipelines.rhs,
            &[
                (0, uniform),
                (1, fields.aperture_u),
                (2, fields.aperture_v),
                (3, fields.predictor_u),
                (4, fields.predictor_v),
                (5, &self.rhs),
                (6, &self.pressure),
                (7, &self.hydrostatic_base),
            ],
        );
        let hydrostatic_bind = (config.gravity_m_s2 != 0.0).then(|| {
            bind(
                device,
                &self.pipelines.hydrostatic,
                &[
                    (0, uniform),
                    (2, fields.aperture_v),
                    (3, &self.hydrostatic_base),
                ],
            )
        });
        let hydrostatic_reference_bind = (config.gravity_m_s2 != 0.0).then(|| {
            bind(
                device,
                &self.pipelines.hydrostatic_reference,
                &[
                    (0, uniform),
                    (1, fields.density),
                    (2, fields.aperture_v),
                    (3, &self.hydrostatic_base),
                ],
            )
        });
        let init_pipeline = if hydrostatic_bind.is_some() {
            &self.pipelines.init_preconditioner_from_guess
        } else {
            &self.pipelines.init_preconditioner
        };
        let init_preconditioner = if hydrostatic_bind.is_some() {
            bind(
                device,
                init_pipeline,
                &[
                    (0, uniform),
                    (1, fields.density),
                    (2, fields.aperture_u),
                    (3, fields.aperture_v),
                    (4, &self.rhs),
                    (12, &self.hydrostatic_base),
                    (5, &self.pressure),
                    (6, &self.residual),
                    (7, &self.preconditioned),
                ],
            )
        } else {
            bind(
                device,
                init_pipeline,
                &[
                    (0, uniform),
                    (1, fields.density),
                    (2, fields.aperture_u),
                    (3, fields.aperture_v),
                    (4, &self.rhs),
                    (5, &self.pressure),
                    (6, &self.residual),
                    (7, &self.preconditioned),
                ],
            )
        };
        let apply_search = bind(
            device,
            &self.pipelines.apply_search,
            &[
                (0, uniform),
                (1, fields.density),
                (2, fields.aperture_u),
                (3, fields.aperture_v),
                (8, &self.search),
                (9, &self.applied_search),
            ],
        );
        let update_solution = bind(
            device,
            &self.pipelines.update_solution,
            &[
                (0, uniform),
                (5, &self.pressure),
                (6, &self.residual),
                (8, &self.search),
                (9, &self.applied_search),
                (11, &self.status),
            ],
        );
        let update_preconditioner = bind(
            device,
            &self.pipelines.update_preconditioner,
            &[
                (0, uniform),
                (1, fields.density),
                (2, fields.aperture_u),
                (3, fields.aperture_v),
                (4, &self.rhs),
                (5, &self.pressure),
                (12, &self.hydrostatic_base),
                (6, &self.residual),
                (7, &self.preconditioned),
            ],
        );
        let update_search = bind(
            device,
            &self.pipelines.update_search,
            &[
                (0, uniform),
                (7, &self.preconditioned),
                (8, &self.search),
                (11, &self.status),
            ],
        );
        let reduce_initial = bind(
            device,
            &self.pipelines.reduce_initial,
            &[
                (0, uniform),
                (6, &self.residual),
                (7, &self.preconditioned),
                (10, &self.partials),
            ],
        );
        let reduce_denominator = bind(
            device,
            &self.pipelines.reduce_denominator,
            &[
                (0, uniform),
                (8, &self.search),
                (9, &self.applied_search),
                (10, &self.partials),
            ],
        );
        let reduce_updated = bind(
            device,
            &self.pipelines.reduce_updated,
            &[
                (0, uniform),
                (1, fields.density),
                (2, fields.aperture_u),
                (3, fields.aperture_v),
                (4, &self.rhs),
                (5, &self.pressure),
                (12, &self.hydrostatic_base),
                (6, &self.residual),
                (10, &self.partials),
            ],
        );
        let finish_initial = bind(
            device,
            &self.pipelines.finish_initial,
            &[(0, uniform), (1, &self.partials), (2, &self.status)],
        );
        let finish_alpha = bind(
            device,
            &self.pipelines.finish_alpha,
            &[(0, uniform), (1, &self.partials), (2, &self.status)],
        );
        let finish_beta = bind(
            device,
            &self.pipelines.finish_beta,
            &[(0, uniform), (1, &self.partials), (2, &self.status)],
        );
        let finish_completion = bind(
            device,
            &self.pipelines.finish_completion,
            &[(0, uniform), (1, &self.partials), (2, &self.status)],
        );
        let correct_u = bind(
            device,
            &self.pipelines.correct_u,
            &[
                (0, uniform),
                (1, fields.density),
                (2, fields.aperture_u),
                (4, fields.predictor_u),
                (6, &self.pressure),
                (9, &self.hydrostatic_base),
                (7, fields.corrected_u),
            ],
        );
        let correct_v = bind(
            device,
            &self.pipelines.correct_v,
            &[
                (0, uniform),
                (1, fields.density),
                (3, fields.aperture_v),
                (5, fields.predictor_v),
                (6, &self.pressure),
                (8, fields.corrected_v),
                (9, &self.hydrostatic_base),
            ],
        );
        let divergence = bind(
            device,
            &self.pipelines.divergence,
            &[
                (0, uniform),
                (1, fields.aperture_u),
                (2, fields.aperture_v),
                (3, fields.corrected_u),
                (4, fields.corrected_v),
                (5, &self.partials),
            ],
        );

        let gauge_reset = bind(
            device,
            &self.pipelines.gauge_reset,
            &[(0, uniform), (2, &self.component_heads)],
        );
        let gauge_link = bind(
            device,
            &self.pipelines.gauge_link,
            &[
                (0, uniform),
                (1, fields.root_cell),
                (2, &self.component_heads),
                (3, &self.component_next),
            ],
        );
        let gauge_mean = bind(
            device,
            &self.pipelines.gauge_mean,
            &[
                (0, uniform),
                (1, fields.root_cell),
                (2, &self.component_heads),
                (3, &self.component_next),
                (4, &self.pressure),
                (5, &self.component_mean),
            ],
        );
        let gauge_center = bind(
            device,
            &self.pipelines.gauge_center,
            &[
                (0, uniform),
                (1, fields.root_cell),
                (4, &self.pressure),
                (5, &self.component_mean),
            ],
        );
        let finish_final_residual = bind(
            device,
            &self.pipelines.finish_final_residual,
            &[(0, uniform), (1, &self.partials), (2, &self.status)],
        );
        PressureBindings {
            rhs_bind,
            hydrostatic_bind,
            hydrostatic_reference_bind,
            init_preconditioner,
            apply_search,
            update_solution,
            update_preconditioner,
            update_search,
            reduce_initial,
            reduce_denominator,
            reduce_updated,
            finish_initial,
            finish_alpha,
            finish_beta,
            finish_completion,
            correct_u,
            correct_v,
            divergence,
            gauge_reset,
            gauge_link,
            gauge_mean,
            gauge_center,
            finish_final_residual,
        }
    }
}
