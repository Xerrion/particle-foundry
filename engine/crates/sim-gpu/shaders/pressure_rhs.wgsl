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
@group(0) @binding(1) var<storage, read> aperture_u: array<f32>;
@group(0) @binding(2) var<storage, read> aperture_v: array<f32>;
@group(0) @binding(3) var<storage, read> predictor_u: array<f32>;
@group(0) @binding(4) var<storage, read> predictor_v: array<f32>;
@group(0) @binding(5) var<storage, read_write> rhs: array<f32>;
@group(0) @binding(6) var<storage, read_write> pressure: array<f32>;
@group(0) @binding(7) var<storage, read_write> hydrostatic_base: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell >= params.cells) {
        return;
    }
    let x = cell % params.width;
    let y = cell / params.width;
    let left = y * (params.width + 1u) + x;
    let right = left + 1u;
    let top = y * params.width + x;
    let bottom = top + params.width;
    let divergence = (
        aperture_u[right] * predictor_u[right]
        - aperture_u[left] * predictor_u[left]
        + aperture_v[bottom] * predictor_v[bottom]
        - aperture_v[top] * predictor_v[top]
    ) / params.dx_m;
    rhs[cell] = -divergence / params.dt_s;
    pressure[cell] = 0.0;
    hydrostatic_base[cell] = 0.0;
}
