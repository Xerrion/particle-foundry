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

struct Status {
    rho: f32,
    alpha: f32,
    beta: f32,
    scaled_residual: f32,
    scaled_divergence: f32,
    iterations: u32,
    converged: u32,
    invalid: u32,
}

struct Partial {
    dot: f32,
    maximum: f32,
    invalid: u32,
    padding: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> density: array<f32>;
@group(0) @binding(2) var<storage, read> aperture_u: array<f32>;
@group(0) @binding(3) var<storage, read> aperture_v: array<f32>;
@group(0) @binding(4) var<storage, read> rhs: array<f32>;
@group(0) @binding(5) var<storage, read_write> pressure: array<f32>;
@group(0) @binding(6) var<storage, read_write> residual: array<f32>;
@group(0) @binding(7) var<storage, read_write> preconditioned: array<f32>;
@group(0) @binding(8) var<storage, read_write> search: array<f32>;
@group(0) @binding(9) var<storage, read_write> applied_search: array<f32>;
@group(0) @binding(10) var<storage, read_write> partials: array<Partial>;
@group(0) @binding(11) var<storage, read> status: Status;
@group(0) @binding(12) var<storage, read> hydrostatic_base: array<f32>;

var<workgroup> shared_dot: array<f32, 64>;
var<workgroup> shared_max: array<f32, 64>;
var<workgroup> shared_invalid: array<u32, 64>;

fn finite(value: f32) -> bool {
    return (bitcast<u32>(value) & 0x7f800000u) != 0x7f800000u;
}

fn edge_weight(aperture: f32, first: u32, second: u32) -> f32 {
    let inverse_density = 1.0 / (0.5 * density[first] + 0.5 * density[second]);
    return aperture * inverse_density / (params.dx_m * params.dx_m);
}

fn apply_search_at(cell: u32, x: u32, y: u32) -> f32 {
    var result = 0.0;
    if (x > 0u) {
        let face = y * (params.width + 1u) + x;
        result += edge_weight(aperture_u[face], cell - 1u, cell)
            * (search[cell] - search[cell - 1u]);
    }
    if (x + 1u < params.width) {
        let face = y * (params.width + 1u) + x + 1u;
        result += edge_weight(aperture_u[face], cell, cell + 1u)
            * (search[cell] - search[cell + 1u]);
    }
    if (y > 0u) {
        let face = y * params.width + x;
        result += edge_weight(aperture_v[face], cell - params.width, cell)
            * (search[cell] - search[cell - params.width]);
    }
    if (y + 1u < params.height) {
        let face = (y + 1u) * params.width + x;
        result += edge_weight(aperture_v[face], cell, cell + params.width)
            * (search[cell] - search[cell + params.width]);
    }
    return result;
}


fn apply_dynamic_at(cell: u32, x: u32, y: u32, offset: u32) -> f32 {
    var result = 0.0;
    if (x > 0u) {
        let face = y * (params.width + 1u) + x;
        result += edge_weight(aperture_u[face], cell - 1u, cell)
            * (pressure[cell + offset] - pressure[cell - 1u + offset]);
    }
    if (x + 1u < params.width) {
        let face = y * (params.width + 1u) + x + 1u;
        result += edge_weight(aperture_u[face], cell, cell + 1u)
            * (pressure[cell + offset] - pressure[cell + 1u + offset]);
    }
    if (y > 0u) {
        let face = y * params.width + x;
        result += edge_weight(aperture_v[face], cell - params.width, cell)
            * (pressure[cell + offset] - pressure[cell - params.width + offset]);
    }
    if (y + 1u < params.height) {
        let face = (y + 1u) * params.width + x;
        result += edge_weight(aperture_v[face], cell, cell + params.width)
            * (pressure[cell + offset] - pressure[cell + params.width + offset]);
    }
    return result;
}

// Keep the small residual contribution separate from the large correction.
// Adding the two pressures before applying the stencil would lose that part.
fn dynamic_residual(cell: u32, x: u32, y: u32, base: f32) -> f32 {
    var r = rhs[cell] - base - apply_dynamic_at(cell, x, y, 0u);
    if (params.padding2 != 0.0) {
        r -= apply_dynamic_at(cell, x, y, params.cells);
    }
    return r;
}

fn two_sum(a: f32, b: f32) -> vec2<f32> {
    let large = select(b, a, abs(a) >= abs(b));
    let small = select(a, b, abs(a) >= abs(b));
    let sum = large + small;
    return vec2<f32>(sum, fma(1.0, small, fma(-1.0, sum, large)));
}

// The base field is stored separately from the small dynamic correction.
// Its vertical face difference uses the row reference density, because subtracting
// two large f32 pressure values would discard the low pressure correction.
fn apply_hydrostatic_at(cell: u32, x: u32, y: u32) -> f32 {
    var result = 0.0;
    if (x > 0u) {
        let face = y * (params.width + 1u) + x;
        result += edge_weight(aperture_u[face], cell - 1u, cell)
            * (hydrostatic_base[cell] - hydrostatic_base[cell - 1u]);
    }
    if (x + 1u < params.width) {
        let face = y * (params.width + 1u) + x + 1u;
        result += edge_weight(aperture_u[face], cell, cell + 1u)
            * (hydrostatic_base[cell] - hydrostatic_base[cell + 1u]);
    }
    if (y > 0u) {
        let face = y * params.width + x;
        if (params.gravity_m_s2 != 0.0) {
            let face_density = 0.5 * density[cell - params.width] + 0.5 * density[cell];
            result += aperture_v[face] * params.gravity_m_s2 / params.dx_m
                * (hydrostatic_base[params.cells + y] / face_density);
        }
    }
    if (y + 1u < params.height) {
        let face = (y + 1u) * params.width + x;
        if (params.gravity_m_s2 != 0.0) {
            let face_density = 0.5 * density[cell] + 0.5 * density[cell + params.width];
            result -= aperture_v[face] * params.gravity_m_s2 / params.dx_m
                * (hydrostatic_base[params.cells + y + 1u] / face_density);
        }
    }
    return result;
}

fn reduce_partial(lane: u32, group: u32, dot: f32, maximum: f32) {
    let invalid = u32(!finite(dot) || !finite(maximum));
    shared_dot[lane] = select(dot, 0.0, invalid != 0u);
    shared_max[lane] = select(maximum, 0.0, invalid != 0u);
    shared_invalid[lane] = invalid;
    workgroupBarrier();
    var span = 32u;
    loop {
        if (span == 0u) {
            break;
        }
        if (lane < span) {
            shared_dot[lane] += shared_dot[lane + span];
            shared_max[lane] = max(shared_max[lane], shared_max[lane + span]);
            shared_invalid[lane] |= shared_invalid[lane + span];
        }
        workgroupBarrier();
        span /= 2u;
    }
    if (lane == 0u) {
        let reduced_dot = shared_dot[0];
        let reduced_max = shared_max[0];
        let invalid_sum = u32(!finite(reduced_dot) || !finite(reduced_max));
        partials[group] = Partial(
            select(reduced_dot, 0.0, invalid_sum != 0u),
            select(reduced_max, 0.0, invalid_sum != 0u),
            shared_invalid[0] | invalid_sum,
            0u,
        );
    }
}

@compute @workgroup_size(64)
fn init_preconditioner(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell >= params.cells) {
        return;
    }
    let r = dynamic_residual(cell, cell % params.width, cell / params.width, 0.0);
    residual[cell] = r;
    preconditioned[cell] = r * density[cell] * params.dx_m * params.dx_m * 0.25;
}

@compute @workgroup_size(64)
fn init_preconditioner_from_guess(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell >= params.cells) {
        return;
    }
    let r = dynamic_residual(cell, cell % params.width, cell / params.width,
        apply_hydrostatic_at(cell, cell % params.width, cell / params.width));
    residual[cell] = r;
    preconditioned[cell] = r * density[cell] * params.dx_m * params.dx_m * 0.25;
}

@compute @workgroup_size(64)
fn apply_search(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell < params.cells) {
        let x = cell % params.width;
        let y = cell / params.width;
        applied_search[cell] = apply_search_at(cell, x, y);
    }
}

@compute @workgroup_size(64)
fn update_solution(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell < params.cells && status.invalid == 0u && status.converged == 0u) {
        let correction = status.alpha * search[cell];
        if (params.padding2 == 0.0) {
            pressure[cell] += correction;
        } else {
            let sum = two_sum(pressure[cell], correction);
            let normalized = two_sum(sum.x, pressure[cell + params.cells] + sum.y);
            pressure[cell] = normalized.x;
            pressure[cell + params.cells] = normalized.y;
        }
        residual[cell] -= status.alpha * applied_search[cell];
    }
}

@compute @workgroup_size(64)
fn update_preconditioner(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell >= params.cells) {
        return;
    }
    let x = cell % params.width;
    let y = cell / params.width;
    let true_residual = dynamic_residual(cell, x, y, apply_hydrostatic_at(cell, x, y));
    residual[cell] = true_residual;
    preconditioned[cell] = residual[cell] * density[cell] * params.dx_m * params.dx_m * 0.25;
}

@compute @workgroup_size(64)
fn update_search(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell < params.cells && status.invalid == 0u && status.converged == 0u) {
        search[cell] = preconditioned[cell] + status.beta * search[cell];
    }
}

@compute @workgroup_size(64)
fn reduce_initial(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    let cell = id.x;
    var dot = 0.0;
    var maximum = 0.0;
    if (cell < params.cells) {
        dot = residual[cell] * preconditioned[cell];
        maximum = abs(residual[cell]);
    }
    reduce_partial(lane, group.x, dot, maximum);
}

@compute @workgroup_size(64)
fn reduce_denominator(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    let cell = id.x;
    var dot = 0.0;
    if (cell < params.cells) {
        dot = search[cell] * applied_search[cell];
    }
    reduce_partial(lane, group.x, dot, 0.0);
}

@compute @workgroup_size(64)
fn reduce_cached(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    let cell = id.x;
    var dot = 0.0;
    var maximum = 0.0;
    if (cell < params.cells) {
        // update_preconditioner just recomputed this true stencil residual.
        // The final acceptance dispatch still recomputes it independently.
        let r = residual[cell];
        let z = preconditioned[cell];
        dot = r * z;
        maximum = abs(r);
    }
    reduce_partial(lane, group.x, dot, maximum);
}

@compute @workgroup_size(64)
fn reduce_updated(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    let cell = id.x;
    var dot = 0.0;
    var maximum = 0.0;
    if (cell < params.cells) {
        let x = cell % params.width;
        let y = cell / params.width;
        let z = residual[cell] * density[cell] * params.dx_m * params.dx_m * 0.25;
        dot = residual[cell] * z;
        maximum = abs(dynamic_residual(cell, x, y, apply_hydrostatic_at(cell, x, y)));
    }
    reduce_partial(lane, group.x, dot, maximum);
}
