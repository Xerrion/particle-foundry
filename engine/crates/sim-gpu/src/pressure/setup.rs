use super::*;

impl GpuPressureProjector {
    /// Allocates only solver scratch after checking effective adapter limits.
    /// Face-density coefficients are recomputed from current density and
    /// aperture buffers on each dispatch, so stale coefficients cannot survive
    /// a fixture step.
    pub(crate) fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        if grid.cell_width_m() != CELL_WIDTH_M {
            return Err("GPU pressure projection requires the base 0.01 m cell width".into());
        }
        let cells = grid.cells() as u32;
        let partial_count = cells.div_ceil(WORKGROUP_SIZE);
        let u_groups = (grid.u_faces() as u32).div_ceil(WORKGROUP_SIZE);
        let v_groups = (grid.v_faces() as u32).div_ceil(WORKGROUP_SIZE);
        let limits = device.limits();
        if WORKGROUP_SIZE > limits.max_compute_invocations_per_workgroup
            || WORKGROUP_SIZE > limits.max_compute_workgroup_size_x
            || partial_count > limits.max_compute_workgroups_per_dimension
            || u_groups > limits.max_compute_workgroups_per_dimension
            || v_groups > limits.max_compute_workgroups_per_dimension
            || limits.max_storage_buffers_per_shader_stage < 8
            || limits.max_bindings_per_bind_group < 13
            || limits.max_uniform_buffer_binding_size < PARAM_BYTES
        {
            return Err("GPU pressure projection exceeds effective adapter limits".into());
        }
        let cell_bytes = u64::from(cells) * size_of::<f32>() as u64;
        let hydrostatic_bytes = cell_bytes + u64::from(grid.height()) * size_of::<f32>() as u64;
        let partial_bytes = u64::from(partial_count) * 16;
        for (name, bytes) in [
            ("pressure cell scratch", cell_bytes),
            ("pressure hydrostatic reference", hydrostatic_bytes),
            ("pressure residual reduction", partial_bytes),
            ("pressure completion", STATUS_BYTES),
        ] {
            if bytes > limits.max_storage_buffer_binding_size || bytes > limits.max_buffer_size {
                return Err(format!("{name} exceeds effective GPU storage limits"));
            }
        }

        let scratch = |label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: cell_bytes,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let rhs = scratch("pressure rhs");
        let hydrostatic_base = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pressure hydrostatic base and face reference"),
            size: hydrostatic_bytes,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let pressure = scratch("pressure dynamic correction");
        let accepted_guess = scratch("accepted pressure starting guess");
        let residual = scratch("pressure recursive residual");
        let preconditioned = scratch("pressure preconditioned residual");
        let search = scratch("pressure search direction");
        let applied_search = scratch("pressure applied search");
        let component_heads = scratch("pressure component list heads");
        let component_next = scratch("pressure component list links");
        let component_mean = scratch("pressure component means");
        let partials = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pressure partial reductions"),
            size: partial_bytes,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let status = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pressure completion status"),
            size: STATUS_BYTES,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let rhs_shader = shader(
            device,
            "pressure rhs",
            include_str!("../../shaders/pressure_rhs.wgsl"),
        );
        let hydrostatic_shader = shader(
            device,
            "pressure hydrostatic initial guess",
            include_str!("../../shaders/pressure_hydrostatic.wgsl"),
        );
        let pcg_shader = shader(
            device,
            "pressure PCG",
            include_str!("../../shaders/pressure_pcg.wgsl"),
        );
        let finish_shader = shader(
            device,
            "pressure finish",
            include_str!("../../shaders/pressure_finish.wgsl"),
        );
        let correct_shader = shader(
            device,
            "pressure correction",
            include_str!("../../shaders/pressure_correct.wgsl"),
        );
        let divergence_shader = shader(
            device,
            "pressure divergence",
            include_str!("../../shaders/pressure_divergence.wgsl"),
        );
        let gauge_shader = shader(
            device,
            "pressure component mean gauge",
            include_str!("../../shaders/pressure_gauge.wgsl"),
        );
        let pipelines = Pipelines {
            rhs: pipeline(device, &rhs_shader, "main"),
            hydrostatic: pipeline(device, &hydrostatic_shader, "main"),
            hydrostatic_reference: pipeline(device, &hydrostatic_shader, "reference"),
            init_preconditioner: pipeline(device, &pcg_shader, "init_preconditioner"),
            init_preconditioner_from_guess: pipeline(
                device,
                &pcg_shader,
                "init_preconditioner_from_guess",
            ),
            apply_search: pipeline(device, &pcg_shader, "apply_search"),
            update_solution: pipeline(device, &pcg_shader, "update_solution"),
            update_preconditioner: pipeline(device, &pcg_shader, "update_preconditioner"),
            update_search: pipeline(device, &pcg_shader, "update_search"),
            reduce_initial: pipeline(device, &pcg_shader, "reduce_initial"),
            reduce_denominator: pipeline(device, &pcg_shader, "reduce_denominator"),
            reduce_updated: pipeline(device, &pcg_shader, "reduce_updated"),
            reduce_cached: pipeline(device, &pcg_shader, "reduce_cached"),
            finish_initial: pipeline(device, &finish_shader, "initial"),
            finish_alpha: pipeline(device, &finish_shader, "alpha"),
            finish_beta: pipeline(device, &finish_shader, "beta"),
            finish_final_residual: pipeline(device, &finish_shader, "final_residual"),
            correct_u: pipeline(device, &correct_shader, "u"),
            correct_v: pipeline(device, &correct_shader, "v"),
            divergence: pipeline(device, &divergence_shader, "main"),
            finish_completion: pipeline(device, &finish_shader, "completion"),
            gauge_reset: pipeline(device, &gauge_shader, "reset"),
            gauge_link: pipeline(device, &gauge_shader, "link"),
            gauge_mean: pipeline(device, &gauge_shader, "mean"),
            gauge_center: pipeline(device, &gauge_shader, "center"),
        };
        Ok(Self {
            grid,
            rhs,
            hydrostatic_base,
            pressure,
            accepted_guess,
            has_accepted_guess: false,
            residual,
            preconditioned,
            search,
            applied_search,
            component_heads,
            component_next,
            component_mean,
            partials,
            status,
            partial_count,
            pipelines,
            last_readback_bytes: 0,
        })
    }
}
