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
@group(0) @binding(1) var<storage, read> partials: array<Partial>;
@group(0) @binding(2) var<storage, read_write> status: Status;

var<workgroup> shared_dot: array<f32, 64>;
var<workgroup> shared_max: array<f32, 64>;
var<workgroup> shared_invalid: array<u32, 64>;

// The corrected face divergence uses different f32 operations from the true
// residual. Reserve headroom before PCG stops; the final gates use the full
// configured tolerances below.
const PCG_STOP_HEADROOM: f32 = 0.5;

fn stop_tolerance() -> f32 {
    return PCG_STOP_HEADROOM * min(params.residual_tolerance, params.divergence_tolerance);
}

fn finite(value: f32) -> bool {
    return (bitcast<u32>(value) & 0x7f800000u) != 0x7f800000u;
}

fn collect(lane: u32) -> Partial {
    var dot = 0.0;
    var maximum = 0.0;
    var invalid = 0u;
    for (var index = lane; index < params.partials; index += 64u) {
        let item = partials[index];
        dot += item.dot;
        maximum = max(maximum, item.maximum);
        invalid |= item.invalid;
    }
    invalid |= u32(!finite(dot) || !finite(maximum));
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
    let final_invalid = shared_invalid[0]
        | u32(!finite(shared_dot[0]) || !finite(shared_max[0]));
    return Partial(shared_dot[0], shared_max[0], final_invalid, 0u);
}

@compute @workgroup_size(64)
fn initial(@builtin(local_invocation_index) lane: u32) {
    let result = collect(lane);
    if (lane == 0u) {
        status.invalid = result.invalid;
        status.rho = result.dot;
        status.scaled_residual = result.maximum * params.dt_s * params.dt_s;
        status.invalid |= u32(!finite(status.scaled_residual) || !finite(status.rho));
        status.converged = u32(
            status.invalid == 0u && status.scaled_residual <= stop_tolerance()
        );
        if (status.converged == 0u && status.rho <= 0.0) {
            status.invalid = 1u;
        }
    }
}

@compute @workgroup_size(64)
fn alpha(@builtin(local_invocation_index) lane: u32) {
    let result = collect(lane);
    if (lane == 0u && status.converged == 0u && status.invalid == 0u) {
        if (result.invalid != 0u || result.dot <= 0.0) {
            status.invalid = 1u;
            return;
        }
        status.alpha = status.rho / result.dot;
        status.invalid = u32(!finite(status.alpha));
    }
}

@compute @workgroup_size(64)
fn beta(@builtin(local_invocation_index) lane: u32) {
    let result = collect(lane);
    if (lane == 0u && status.converged == 0u && status.invalid == 0u) {
        status.iterations += 1u;
        status.scaled_residual = result.maximum * params.dt_s * params.dt_s;
        status.invalid = result.invalid | u32(!finite(status.scaled_residual));
        if (status.invalid != 0u) {
            return;
        }
        status.converged = u32(status.scaled_residual <= stop_tolerance());
        if (status.converged != 0u) {
            status.beta = 0.0;
            return;
        }
        if (result.dot <= 0.0 || status.rho <= 0.0) {
            status.invalid = 1u;
            return;
        }
        status.beta = result.dot / status.rho;
        status.rho = result.dot;
        status.invalid = u32(!finite(status.beta));
    }
}

@compute @workgroup_size(64)
fn final_residual(@builtin(local_invocation_index) lane: u32) {
    let result = collect(lane);
    if (lane == 0u) {
        status.scaled_residual = result.maximum * params.dt_s * params.dt_s;
        status.invalid |= result.invalid | u32(!finite(status.scaled_residual));
        status.converged = u32(
            status.invalid == 0u
            && status.converged != 0u
            && status.scaled_residual <= params.residual_tolerance
        );
    }
}

@compute @workgroup_size(64)
fn completion(@builtin(local_invocation_index) lane: u32) {
    let result = collect(lane);
    if (lane == 0u) {
        status.scaled_divergence = result.maximum * params.dt_s;
        status.invalid |= result.invalid | u32(!finite(status.scaled_divergence));
        status.converged = u32(
            status.invalid == 0u
            && status.converged != 0u
            && status.scaled_divergence <= params.divergence_tolerance
        );
    }
}
