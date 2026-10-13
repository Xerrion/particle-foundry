use super::*;
use wgpu::util::DeviceExt;

impl CoarsePressure {
    pub(in crate::pressure) fn new(
        device: &wgpu::Device,
        grid: Grid,
        status: &wgpu::Buffer,
        residual: &wgpu::Buffer,
        output: &wgpu::Buffer,
    ) -> Result<Self, String> {
        let module = |label, source: &str| {
            let source = format!(
                "{}\n{source}",
                include_str!("../../../shaders/pressure_coarse_node.wgsl")
            );
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            })
        };
        let fine = module(
            "coarse pressure fine coefficients",
            include_str!("../../../shaders/pressure_coarse_fine.wgsl"),
        );
        let nodes = module(
            "coarse pressure Galerkin coefficients",
            include_str!("../../../shaders/pressure_coarse_nodes.wgsl"),
        );
        let cycle = module(
            "coarse pressure symmetric cycle",
            include_str!("../../../shaders/pressure_coarse_cycle.wgsl"),
        );
        let stages = Stages {
            fine: pipeline(device, &fine, "main"),
            roots: pipeline(device, &nodes, "roots"),
            weights: pipeline(device, &nodes, "weights"),
            initialize: pipeline(device, &cycle, "initialize"),
            smooth: pipeline(device, &cycle, "relax"),
            restrict: pipeline(device, &cycle, "restrict_residual"),
            prolong: pipeline(device, &cycle, "prolong"),
            export: pipeline(device, &cycle, "export_correction"),
        };
        let mut levels = Vec::<Level>::new();
        let mut width = grid.width();
        let mut height = grid.height();
        loop {
            let cells = u64::from(width) * u64::from(height);
            let groups = (cells as u32).div_ceil(WORKGROUP_SIZE);
            let limits = device.limits();
            if cells * NODE_BYTES > limits.max_buffer_size
                || cells * NODE_BYTES > limits.max_storage_buffer_binding_size
                || groups > limits.max_compute_workgroups_per_dimension
            {
                return Err("coarse pressure hierarchy exceeds effective GPU limits".into());
            }
            let buffer = |label, size| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size,
                    usage: wgpu::BufferUsages::STORAGE,
                    mapped_at_creation: false,
                })
            };
            let previous = levels
                .last()
                .map(|l| (l.width, l.height))
                .unwrap_or((width, height));
            let mut bytes = Vec::new();
            for value in [previous.0, previous.1, width, height] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            for value in [grid.cell_width_m() as f32, 2.0 / 3.0, 0.0, 0.0] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            levels.push(Level {
                width,
                height,
                groups,
                uniform: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("coarse pressure level geometry"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::UNIFORM,
                }),
                nodes: buffer(
                    "coarse pressure node coefficients and roots",
                    cells * NODE_BYTES,
                ),
                rhs: buffer("coarse pressure level residual", cells * 4),
                first: buffer("coarse pressure correction", cells * 4),
                second: buffer("coarse pressure smoothing scratch", cells * 4),
            });
            if width == 1 && height == 1 {
                break;
            }
            width = width.div_ceil(2);
            height = height.div_ceil(2);
        }
        let smoothing = levels
            .iter()
            .map(|l| {
                let stage_bind = |source, destination| {
                    bind(
                        device,
                        &stages.smooth,
                        &[
                            (0, &l.uniform),
                            (1, &l.nodes),
                            (3, &l.rhs),
                            (4, source),
                            (5, destination),
                            (8, status),
                        ],
                    )
                };
                LevelBindings {
                    forward: stage_bind(&l.first, &l.second),
                    backward: stage_bind(&l.second, &l.first),
                }
            })
            .collect();
        let transfers = levels
            .windows(2)
            .map(|pair| {
                let (f, c) = (&pair[0], &pair[1]);
                let nodes = [(0, &c.uniform), (1, &f.nodes), (2, &c.nodes)];
                TransferBindings {
                    roots: bind(device, &stages.roots, &nodes),
                    weights: bind(device, &stages.weights, &nodes),
                    restrict: bind(
                        device,
                        &stages.restrict,
                        &[
                            (0, &c.uniform),
                            (1, &f.nodes),
                            (2, &c.nodes),
                            (3, &f.rhs),
                            (4, &f.first),
                            (6, &c.rhs),
                            (7, &c.first),
                            (8, status),
                        ],
                    ),
                    prolong: bind(
                        device,
                        &stages.prolong,
                        &[
                            (0, &c.uniform),
                            (1, &f.nodes),
                            (2, &c.nodes),
                            (4, &f.first),
                            (7, &c.first),
                            (8, status),
                        ],
                    ),
                }
            })
            .collect();
        let fine = &levels[0];
        let initialize = bind(
            device,
            &stages.initialize,
            &[
                (0, &fine.uniform),
                (3, &fine.rhs),
                (4, &fine.first),
                (8, status),
                (9, residual),
            ],
        );
        let export = bind(
            device,
            &stages.export,
            &[
                (0, &fine.uniform),
                (4, &fine.first),
                (8, status),
                (10, output),
            ],
        );
        Ok(Self {
            levels,
            stages,
            smoothing,
            transfers,
            initialize,
            export,
        })
    }
}
