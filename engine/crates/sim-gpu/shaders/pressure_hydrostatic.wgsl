struct Params {
    width: u32,
    height: u32,
    cells: u32,
    partials: u32,
    dx_m: f32,
    dt_s: f32,
    residual_tolerance: f32,
    divergence_tolerance: f32,
    gravity_m_s2: f32,
    padding0: f32,
    padding1: f32,
    padding2: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> density: array<f32>;
@group(0) @binding(2) var<storage, read> aperture_v: array<f32>;
@group(0) @binding(3) var<storage, read_write> hydrostatic_base: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let x = id.x;
    if (x >= params.width) {
        return;
    }
    var column_pressure = 0.0;
    hydrostatic_base[x] = 0.0;
    for (var y = 1u; y < params.height; y += 1u) {
        let bottom = y * params.width + x;
        let top = bottom - params.width;
        if (aperture_v[bottom] > 0.0) {
            let face_density = 0.5 * density[top] + 0.5 * density[bottom];
            column_pressure += face_density * params.gravity_m_s2 * params.dx_m;
        } else {
            column_pressure = 0.0;
        }
        hydrostatic_base[bottom] = column_pressure;
    }
}
