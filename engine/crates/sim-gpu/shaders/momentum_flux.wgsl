struct Params {
    width: u32,
    height: u32,
    cells: u32,
    reserved: u32,
}

struct FaceFlux {
    volume: f32,
    liquid_mass: f32,
    carrier_mass: f32,
    liquid_marker: f32,
    carrier_marker: f32,
}

// Three scalar fields have a 12-byte storage stride. A vec3 would not.
struct DualFlux {
    mass0: f32,
    mass1: f32,
    momentum: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> primal: array<FaceFlux>;
@group(0) @binding(2) var<storage, read> velocity: array<f32>;
@group(0) @binding(3) var<storage, read_write> dual: array<DualFlux>;
@group(0) @binding(4) var<storage, read_write> status: array<atomic<u32>>;

const INVALID_LEDGER: u32 = 1u;

fn finite(value: f32) -> bool {
    return value == value && value <= 3.402823e38 && value >= -3.402823e38;
}

fn zero_flux() -> DualFlux {
    return DualFlux(0.0, 0.0, 0.0);
}

fn mass(face: u32) -> f32 {
    let record = primal[face];
    return record.liquid_mass + record.carrier_mass;
}

fn upwind(signed_mass: f32, negative_speed: f32, positive_speed: f32) -> f32 {
    if signed_mass > 0.0 {
        return signed_mass * negative_speed;
    }
    if signed_mass < 0.0 {
        return signed_mass * positive_speed;
    }
    return 0.0;
}

fn paired_flux(first: f32, second: f32, aligned: bool,
               negative_speed: f32, positive_speed: f32) -> DualFlux {
    if !(finite(first) && finite(second) && finite(negative_speed) && finite(positive_speed)) {
        atomicOr(&status[0], INVALID_LEDGER);
        return zero_flux();
    }
    // Aligned dual edges first combine the two primal half faces. Transverse
    // edges keep opposite signed halves separate until after upwinding.
    var result = DualFlux(first, second, 0.0);
    if aligned {
        result.mass0 = first + second;
        result.mass1 = 0.0;
    }
    result.momentum = upwind(result.mass0, negative_speed, positive_speed)
                    + upwind(result.mass1, negative_speed, positive_speed);
    if !(finite(result.mass0) && finite(result.mass1) && finite(result.momentum)) {
        atomicOr(&status[0], INVALID_LEDGER);
        return zero_flux();
    }
    return result;
}

@compute @workgroup_size(64)
fn ux(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let id = global_id.x;
    let stride = params.width + 2u;
    if id >= stride * params.height { return; }
    let x = id % stride;
    let y = id / stride;
    dual[id] = zero_flux();
    if x == 0u || x == params.width + 1u { return; }
    let first = y * (params.width + 1u) + x - 1u;
    let negative = y * (params.width + 1u) + x - 1u;
    let positive = negative + 1u;
    dual[id] = paired_flux(0.5 * mass(first), 0.5 * mass(first + 1u), true,
                           velocity[negative], velocity[positive]);
}

@compute @workgroup_size(64)
fn vx(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let id = global_id.x;
    let stride = params.width + 1u;
    if id >= stride * (params.height + 1u) { return; }
    let x = id % stride;
    let y = id / stride;
    dual[id] = zero_flux();
    if x == 0u || x == params.width { return; }
    var first = 0.0;
    var second = 0.0;
    if y > 0u { first = 0.5 * mass((y - 1u) * stride + x); }
    if y < params.height { second = 0.5 * mass(y * stride + x); }
    let negative = y * params.width + x - 1u;
    dual[id] = paired_flux(first, second, false,
                           velocity[negative], velocity[negative + 1u]);
}

@compute @workgroup_size(64)
fn uy(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let id = global_id.x;
    let stride = params.width + 1u;
    if id >= stride * (params.height + 1u) { return; }
    let x = id % stride;
    let y = id / stride;
    dual[id] = zero_flux();
    if y == 0u || y == params.height { return; }
    var first = 0.0;
    var second = 0.0;
    if x > 0u { first = 0.5 * mass(y * params.width + x - 1u); }
    if x < params.width { second = 0.5 * mass(y * params.width + x); }
    let negative = (y - 1u) * stride + x;
    dual[id] = paired_flux(first, second, false,
                           velocity[negative], velocity[negative + stride]);
}

@compute @workgroup_size(64)
fn vy(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let id = global_id.x;
    let stride = params.width;
    if id >= stride * (params.height + 2u) { return; }
    let x = id % stride;
    let y = id / stride;
    dual[id] = zero_flux();
    if y == 0u || y == params.height + 1u { return; }
    let first = (y - 1u) * stride + x;
    dual[id] = paired_flux(0.5 * mass(first), 0.5 * mass(first + stride), true,
                           velocity[first], velocity[first + stride]);
}
