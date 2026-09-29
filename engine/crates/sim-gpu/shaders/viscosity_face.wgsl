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
@group(0) @binding(2) var<storage, read> velocity: array<f32>;
@group(0) @binding(3) var<storage, read> aperture: array<f32>;
@group(0) @binding(4) var<storage, read> corner_stress: array<f32>;
@group(0) @binding(5) var<storage, read_write> candidate_velocity: array<f32>;
@group(0) @binding(6) var<storage, read_write> status: array<atomic<u32>>;

const INVALID_CELL: u32 = 1u;
const INVALID_FACE: u32 = 2u;
const NONFINITE_RESULT: u32 = 4u;

fn finite(value: f32) -> bool {
    return value == value && value <= 3.402823e38 && value >= -3.402823e38;
}

fn face(id: u32, axis: u32) {
    let face_count = select((params.width + 1u) * params.height,
                            params.width * (params.height + 1u), axis == 1u);
    if id >= face_count {
        return;
    }
    candidate_velocity[id] = 0.0;
    let stride = select(params.width + 1u, params.width, axis == 1u);
    let x = id % stride;
    let y = id / stride;
    let outer = select(x == 0u || x == params.width,
                       y == 0u || y == params.height, axis == 1u);
    let speed = velocity[id];
    let opening = aperture[id];
    if !(finite(speed) && finite(opening) && opening >= 0.0 && opening <= 1.0) {
        atomicOr(&status[0], INVALID_FACE);
        return;
    }
    if outer || opening == 0.0 {
        if speed != 0.0 || (outer && opening != 0.0) {
            atomicOr(&status[0], INVALID_FACE);
        }
        return;
    }

    let negative_cell = select((y - 1u) * params.width + x,
                               y * params.width + x - 1u, axis == 0u);
    let positive_cell = y * params.width + x;
    let rho_negative = density[negative_cell];
    let rho_positive = density[positive_cell];
    if !(finite(rho_negative) && finite(rho_positive)
         && rho_negative >= params.minimum_density
         && rho_positive >= params.minimum_density) {
        atomicOr(&status[0], INVALID_CELL);
        return;
    }
    let rho = rho_negative + (rho_positive - rho_negative) * 0.5;
    var stress_before = 0.0;
    var stress_after = 0.0;
    if axis == 0u {
        // This face is below the corner at y and above the corner at y+1.
        if y > 0u { stress_after = corner_stress[y * params.width + x]; }
        if y + 1u < params.height {
            stress_before = corner_stress[(y + 1u) * params.width + x];
        }
    } else {
        if x + 1u < params.width {
            stress_before = corner_stress[y * params.width + x + 1u];
        }
        if x > 0u { stress_after = corner_stress[y * params.width + x]; }
    }
    let impulse = params.dt_s * (stress_before - stress_after)
                  / (rho * params.cell_width_m);
    let next = speed + impulse;
    if !(finite(rho) && rho > 0.0 && finite(impulse) && finite(next)) {
        atomicOr(&status[0], NONFINITE_RESULT);
        return;
    }
    candidate_velocity[id] = next;
}

@compute @workgroup_size(64)
fn u_faces(@builtin(global_invocation_id) global_id: vec3<u32>) {
    face(global_id.x, 0u);
}

@compute @workgroup_size(64)
fn v_faces(@builtin(global_invocation_id) global_id: vec3<u32>) {
    face(global_id.x, 1u);
}
