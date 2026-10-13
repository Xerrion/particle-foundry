struct Params {
    width: u32, height: u32, cells: u32, partials: u32,
    dx_m: f32, dt_s: f32, residual_tolerance: f32, divergence_tolerance: f32,
    gravity_m_s2: f32, padding0: f32, padding1: f32, padding2: f32,
}
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> density: array<f32>;
@group(0) @binding(2) var<storage, read> aperture_u: array<f32>;
@group(0) @binding(3) var<storage, read> aperture_v: array<f32>;
@group(0) @binding(4) var<storage, read> root_cell: array<u32>;
@group(0) @binding(5) var<storage, read_write> nodes: array<Node>;

fn edge(aperture: f32, a: u32, b: u32) -> f32 {
    return aperture * (1.0 / (0.5 * density[a] + 0.5 * density[b]))
        / (params.dx_m * params.dx_m);
}
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell >= params.cells) { return; }
    let x = cell % params.width;
    let y = cell / params.width;
    var edges = vec4<f32>(0.0);
    if (x > 0u) { edges.x = edge(aperture_u[y * (params.width + 1u) + x], cell - 1u, cell); }
    if (x + 1u < params.width) { edges.y = edge(aperture_u[y * (params.width + 1u) + x + 1u], cell, cell + 1u); }
    if (y > 0u) { edges.z = edge(aperture_v[y * params.width + x], cell - params.width, cell); }
    if (y + 1u < params.height) { edges.w = edge(aperture_v[(y + 1u) * params.width + x], cell, cell + params.width); }
    let diagonal = edges.x + edges.y + edges.z + edges.w;
    nodes[cell] = Node(edges, diagonal, select(0xffffffffu, root_cell[cell], diagonal > 0.0),
        u32(diagonal > 0.0), 0u);
}
