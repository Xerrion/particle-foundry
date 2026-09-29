struct Params {
    width: u32,
    height: u32,
    cells: u32,
    reserved: u32,
}

struct DualFlux {
    mass0: f32,
    mass1: f32,
    momentum: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> before_mass: array<f32>;
@group(0) @binding(2) var<storage, read> after_mass: array<f32>;
@group(0) @binding(3) var<storage, read> before_velocity: array<f32>;
@group(0) @binding(4) var<storage, read> aperture: array<f32>;
@group(0) @binding(5) var<storage, read> dual: array<DualFlux>;
@group(0) @binding(6) var<storage, read_write> after_velocity: array<f32>;
@group(0) @binding(7) var<storage, read_write> wall_impulse: array<f32>;
@group(0) @binding(8) var<storage, read_write> status: array<atomic<u32>>;

const INVALID_LEDGER: u32 = 1u;
const DONOR_OVERDRAW: u32 = 2u;
const EMPTY_OPEN_FACE: u32 = 4u;
const REL_TOL: f32 = 2e-5;

fn finite(value: f32) -> bool {
    return value == value && value <= 3.402823e38 && value >= -3.402823e38;
}

fn cell_mass(index: u32, after: bool) -> f32 {
    if after {
        return after_mass[index] + after_mass[params.cells + index];
    }
    return before_mass[index] + before_mass[params.cells + index];
}

fn dual_mass(x: u32, y: u32, component: u32, after: bool) -> f32 {
    var result = 0.0;
    if component == 0u {
        if x > 0u { result += 0.5 * cell_mass(y * params.width + x - 1u, after); }
        if x < params.width { result += 0.5 * cell_mass(y * params.width + x, after); }
    } else {
        if y > 0u { result += 0.5 * cell_mass((y - 1u) * params.width + x, after); }
        if y < params.height { result += 0.5 * cell_mass(y * params.width + x, after); }
    }
    return result;
}

fn outgoing(value: DualFlux, positive_edge: bool) -> f32 {
    if positive_edge {
        return max(0.0, value.mass0) + max(0.0, value.mass1);
    }
    return max(0.0, -value.mass0) + max(0.0, -value.mass1);
}

fn gather(id: u32, component: u32, sweep: u32) {
    let width = select(params.width + 1u, params.width, component == 1u);
    let height = select(params.height, params.height + 1u, component == 1u);
    if id >= width * height { return; }
    let x = id % width;
    let y = id / width;
    let negative_edge = select(y * (width + 1u) + x, y * width + x, sweep == 1u);
    let positive_edge = select(negative_edge + 1u, negative_edge + width, sweep == 1u);
    let negative = dual[negative_edge];
    let positive = dual[positive_edge];
    let before = dual_mass(x, y, component, false);
    let after = dual_mass(x, y, component, true);
    let in_mass = negative.mass0 + negative.mass1;
    let out_mass = positive.mass0 + positive.mass1;
    let predicted = before + in_mass - out_mass;
    let available = before * (1.0 + REL_TOL);
    let sent = outgoing(negative, false) + outgoing(positive, true);
    let scale = max(max(abs(before), abs(after)), max(abs(in_mass), abs(out_mass)));
    after_velocity[id] = 0.0;
    wall_impulse[id] = 0.0;
    if !(finite(before) && finite(after) && finite(predicted) && finite(sent)
         && finite(aperture[id]) && aperture[id] >= 0.0 && aperture[id] <= 1.0
         && finite(before_velocity[id]) && before >= 0.0 && after >= 0.0
         && abs(after - predicted) <= REL_TOL * scale) {
        atomicOr(&status[0], INVALID_LEDGER);
        return;
    }
    if sent > available {
        atomicOr(&status[0], DONOR_OVERDRAW);
        return;
    }
    if aperture[id] != 0.0 && (before <= 0.0 || after <= 0.0) {
        atomicOr(&status[0], EMPTY_OPEN_FACE);
        return;
    }
    let momentum = before * before_velocity[id] + negative.momentum - positive.momentum;
    if !finite(momentum) {
        atomicOr(&status[0], INVALID_LEDGER);
        return;
    }
    if aperture[id] == 0.0 {
        wall_impulse[id] = -momentum;
    } else {
        after_velocity[id] = momentum / after;
    }
    if !(finite(after_velocity[id]) && finite(wall_impulse[id])) {
        atomicOr(&status[0], INVALID_LEDGER);
        after_velocity[id] = 0.0;
        wall_impulse[id] = 0.0;
    }
}

@compute @workgroup_size(64)
fn ux(@builtin(global_invocation_id) global_id: vec3<u32>) {
    gather(global_id.x, 0u, 0u);
}

@compute @workgroup_size(64)
fn vx(@builtin(global_invocation_id) global_id: vec3<u32>) {
    gather(global_id.x, 1u, 0u);
}

@compute @workgroup_size(64)
fn uy(@builtin(global_invocation_id) global_id: vec3<u32>) {
    gather(global_id.x, 0u, 1u);
}

@compute @workgroup_size(64)
fn vy(@builtin(global_invocation_id) global_id: vec3<u32>) {
    gather(global_id.x, 1u, 1u);
}
