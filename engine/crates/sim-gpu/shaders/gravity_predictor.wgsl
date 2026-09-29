struct Params {
    width: u32,
    height: u32,
    cells: u32,
    reserved: u32,
    dt_s: f32,
    gravity_m_s2: f32,
    reserved0: f32,
    reserved1: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_velocity: array<f32>;
@group(0) @binding(2) var<storage, read> aperture: array<f32>;
@group(0) @binding(3) var<storage, read_write> predictor: array<f32>;
@group(0) @binding(4) var<storage, read_write> status: array<atomic<u32>>;

const INVALID_FACE: u32 = 1u;
const NONFINITE_RESULT: u32 = 2u;

fn finite(value: f32) -> bool {
    return value == value && value <= 3.402823e38 && value >= -3.402823e38;
}

fn predict(id: u32, axis: u32) {
    let count = select((params.width + 1u) * params.height,
                       params.width * (params.height + 1u), axis == 1u);
    if id >= count { return; }
    predictor[id] = 0.0;
    let stride = select(params.width + 1u, params.width, axis == 1u);
    let x = id % stride;
    let y = id / stride;
    let outer = select(x == 0u || x == params.width,
                       y == 0u || y == params.height, axis == 1u);
    let open = aperture[id];
    let speed = input_velocity[id];
    if !(finite(open) && open >= 0.0 && open <= 1.0 && finite(speed)) {
        atomicOr(&status[0], INVALID_FACE);
        return;
    }
    if outer || open == 0.0 {
        if speed != 0.0 || (outer && open != 0.0) {
            atomicOr(&status[0], INVALID_FACE);
        }
        return;
    }
    var next = speed;
    if axis == 1u {
        next += params.dt_s * params.gravity_m_s2;
    }
    if !finite(next) {
        atomicOr(&status[0], NONFINITE_RESULT);
        return;
    }
    predictor[id] = next;
}

@compute @workgroup_size(64)
fn u_faces(@builtin(global_invocation_id) global_id: vec3<u32>) {
    predict(global_id.x, 0u);
}

@compute @workgroup_size(64)
fn v_faces(@builtin(global_invocation_id) global_id: vec3<u32>) {
    predict(global_id.x, 1u);
}
