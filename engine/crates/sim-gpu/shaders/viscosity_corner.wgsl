struct Params {
    width: u32,
    height: u32,
    cells: u32,
    reserved: u32,
    dt_s: f32,
    minimum_density: f32,
    maximum_viscosity: f32,
    cell_width_m: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> density: array<f32>;
@group(0) @binding(2) var<storage, read> viscosity: array<f32>;
@group(0) @binding(3) var<storage, read> u_velocity: array<f32>;
@group(0) @binding(4) var<storage, read> v_velocity: array<f32>;
@group(0) @binding(5) var<storage, read> u_aperture: array<f32>;
@group(0) @binding(6) var<storage, read> v_aperture: array<f32>;
@group(0) @binding(7) var<storage, read_write> corner_stress: array<f32>;
@group(0) @binding(8) var<storage, read_write> status: array<atomic<u32>>;

const INVALID_CELL: u32 = 1u;
const INVALID_FACE: u32 = 2u;
const NONFINITE_RESULT: u32 = 4u;

fn finite(value: f32) -> bool {
    return value == value && value <= 3.402823e38 && value >= -3.402823e38;
}

@compute @workgroup_size(64)
fn corners(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let id = global_id.x;
    if id >= params.cells {
        return;
    }
    corner_stress[id] = 0.0;
    let rho = density[id];
    let mu = viscosity[id];
    if !(finite(rho) && rho >= params.minimum_density && finite(mu)
         && mu > 0.0 && mu <= params.maximum_viscosity) {
        atomicOr(&status[0], INVALID_CELL);
        return;
    }
    let x = id % params.width;
    let y = id / params.width;
    if x == 0u || y == 0u {
        return;
    }

    let above_y = y - 1u;
    let left_x = x - 1u;
    let u_above = above_y * (params.width + 1u) + x;
    let u_below = y * (params.width + 1u) + x;
    let v_left = y * params.width + left_x;
    let v_right = y * params.width + x;
    let openings = vec4<f32>(u_aperture[u_above], u_aperture[u_below],
                             v_aperture[v_left], v_aperture[v_right]);
    let speeds = vec4<f32>(u_velocity[u_above], u_velocity[u_below],
                           v_velocity[v_left], v_velocity[v_right]);
    if !(all(openings >= vec4<f32>(0.0)) && all(openings <= vec4<f32>(1.0))
         && finite(openings.x) && finite(openings.y)
         && finite(openings.z) && finite(openings.w)
         && finite(speeds.x) && finite(speeds.y)
         && finite(speeds.z) && finite(speeds.w)) {
        atomicOr(&status[0], INVALID_FACE);
        return;
    }
    let opening = min(min(openings.x, openings.y), min(openings.z, openings.w));
    if opening == 0.0 {
        return;
    }
    let upper_left = above_y * params.width + left_x;
    let upper_right = above_y * params.width + x;
    let lower_left = y * params.width + left_x;
    let lower_right = y * params.width + x;
    let mu_corners = vec4<f32>(viscosity[upper_left], viscosity[upper_right],
                               viscosity[lower_left], viscosity[lower_right]);
    let quarter_mu = mu_corners * 0.25;
    let mean_mu = (quarter_mu.x + quarter_mu.y) + (quarter_mu.z + quarter_mu.w);
    let shear_rate = (speeds.y - speeds.x + speeds.w - speeds.z) / params.cell_width_m;
    let stress = mean_mu * opening * shear_rate;
    if !(finite(mean_mu) && finite(shear_rate) && finite(stress)) {
        atomicOr(&status[0], NONFINITE_RESULT);
        return;
    }
    corner_stress[id] = stress;
}
