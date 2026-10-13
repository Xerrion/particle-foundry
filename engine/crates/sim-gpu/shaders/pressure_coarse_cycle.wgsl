struct Params {
    previous_width: u32, previous_height: u32, width: u32, height: u32,
    dx_m: f32, damping: f32, padding0: f32, padding1: f32,
}
struct Status {
    rho: f32, alpha: f32, beta: f32, scaled_residual: f32,
    scaled_divergence: f32, iterations: u32, converged: u32, invalid: u32,
}
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> nodes: array<Node>;
@group(0) @binding(2) var<storage, read> parents: array<Node>;
@group(0) @binding(3) var<storage, read_write> rhs: array<f32>;
@group(0) @binding(4) var<storage, read_write> correction: array<f32>;
@group(0) @binding(5) var<storage, read_write> next_correction: array<f32>;
@group(0) @binding(6) var<storage, read_write> parent_rhs: array<f32>;
@group(0) @binding(7) var<storage, read_write> parent_correction: array<f32>;
@group(0) @binding(8) var<storage, read> status: Status;
@group(0) @binding(9) var<storage, read> residual: array<f32>;
@group(0) @binding(10) var<storage, read_write> output: array<f32>;

fn stopped() -> bool { return status.invalid != 0u || status.converged != 0u; }

fn applied(cell: u32, width: u32, height: u32) -> f32 {
    let x = cell % width;
    let y = cell / width;
    let node = nodes[cell];
    let e = node.edges;
    let value = correction[cell];
    var result = max(node.diagonal - (e.x + e.y + e.z + e.w), 0.0) * value;
    if (x > 0u) { result += e.x * (value - correction[cell - 1u]); }
    if (x + 1u < width) { result += e.y * (value - correction[cell + 1u]); }
    if (y > 0u) { result += e.z * (value - correction[cell - width]); }
    if (y + 1u < height) { result += e.w * (value - correction[cell + width]); }
    return result;
}

@compute @workgroup_size(64)
fn initialize(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell >= params.width * params.height || stopped()) { return; }
    rhs[cell] = residual[cell];
    correction[cell] = 0.0;
}

@compute @workgroup_size(64)
fn relax(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell >= params.width * params.height || stopped()) { return; }
    let diagonal = nodes[cell].diagonal;
    var value = 0.0;
    if (diagonal > 0.0) {
        value = correction[cell] + params.damping
            * (rhs[cell] - applied(cell, params.width, params.height)) / diagonal;
    }
    next_correction[cell] = value;
}

@compute @workgroup_size(64)
fn restrict_residual(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell >= params.width * params.height || stopped()) { return; }
    let root = parents[cell].root;
    let x = 2u * (cell % params.width);
    let y = 2u * (cell / params.width);
    var sum = 0.0;
    for (var dy = 0u; dy < 2u; dy += 1u) {
        for (var dx = 0u; dx < 2u; dx += 1u) {
            if (x + dx < params.previous_width && y + dy < params.previous_height) {
                let child = (y + dy) * params.previous_width + x + dx;
                if (nodes[child].root == root && nodes[child].count != 0u) {
                    sum += rhs[child] - applied(child, params.previous_width, params.previous_height);
                }
            }
        }
    }
    parent_rhs[cell] = sum;
    parent_correction[cell] = 0.0;
}

@compute @workgroup_size(64)
fn prolong(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell >= params.previous_width * params.previous_height || stopped()) { return; }
    let x = cell % params.previous_width;
    let y = cell / params.previous_width;
    let parent = (y / 2u) * params.width + x / 2u;
    if (nodes[cell].root == parents[parent].root && nodes[cell].count != 0u) {
        correction[cell] += parent_correction[parent];
    }
}

@compute @workgroup_size(64)
fn export_correction(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell < params.width * params.height && !stopped()) {
        output[cell] = correction[cell];
    }
}
