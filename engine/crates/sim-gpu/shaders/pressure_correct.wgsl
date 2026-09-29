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
@group(0) @binding(2) var<storage, read> aperture_u: array<f32>;
@group(0) @binding(3) var<storage, read> aperture_v: array<f32>;
@group(0) @binding(4) var<storage, read> predictor_u: array<f32>;
@group(0) @binding(5) var<storage, read> predictor_v: array<f32>;
@group(0) @binding(6) var<storage, read> pressure: array<f32>;
@group(0) @binding(7) var<storage, read_write> corrected_u: array<f32>;
@group(0) @binding(8) var<storage, read_write> corrected_v: array<f32>;
@group(0) @binding(9) var<storage, read> hydrostatic_base: array<f32>;

@compute @workgroup_size(64)
fn u(@builtin(global_invocation_id) id: vec3<u32>) {
    let face = id.x;
    let face_count = (params.width + 1u) * params.height;
    if (face >= face_count) {
        return;
    }
    let x = face % (params.width + 1u);
    if (x == 0u || x == params.width || aperture_u[face] == 0.0) {
        corrected_u[face] = 0.0;
        return;
    }
    let left = (face / (params.width + 1u)) * params.width + x - 1u;
    let right = left + 1u;
    let inverse_density = 1.0 / (0.5 * density[left] + 0.5 * density[right]);
    let base_gradient = (hydrostatic_base[right] - hydrostatic_base[left]) / params.dx_m;
    let dynamic_gradient = (pressure[right] - pressure[left]) / params.dx_m;
    corrected_u[face] = predictor_u[face]
        - params.dt_s * inverse_density * base_gradient
        - params.dt_s * inverse_density * dynamic_gradient;
}

@compute @workgroup_size(64)
fn v(@builtin(global_invocation_id) id: vec3<u32>) {
    let face = id.x;
    let face_count = params.width * (params.height + 1u);
    if (face >= face_count) {
        return;
    }
    let y = face / params.width;
    if (y == 0u || y == params.height || aperture_v[face] == 0.0) {
        corrected_v[face] = 0.0;
        return;
    }
    let top = (y - 1u) * params.width + face % params.width;
    let bottom = top + params.width;
    let inverse_density = 1.0 / (0.5 * density[top] + 0.5 * density[bottom]);
    let dynamic_gradient = (pressure[bottom] - pressure[top]) / params.dx_m;
    corrected_v[face] = predictor_v[face] - params.dt_s * params.gravity_m_s2
        - params.dt_s * inverse_density * dynamic_gradient;
}
