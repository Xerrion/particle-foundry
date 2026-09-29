struct Params {
    width: u32,
    height: u32,
    cells: u32,
    reserved: u32,
    dt_s: f32,
    liquid_density: f32,
    carrier_density: f32,
    cell_volume: f32,
}

struct CellHeader {
    energy_j: f32,
    fixed_wall: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> headers: array<CellHeader>;
@group(0) @binding(2) var<storage, read> u_aperture: array<f32>;
@group(0) @binding(3) var<storage, read> v_aperture: array<f32>;
@group(0) @binding(4) var<storage, read_write> residual: array<f32>;
@group(0) @binding(5) var<storage, read_write> u_velocity: array<f32>;
@group(0) @binding(6) var<storage, read_write> v_velocity: array<f32>;
@group(0) @binding(7) var<storage, read_write> status: array<atomic<u32>>;

const CORRECTION_LIMIT: u32 = 16u;
const CELL_WIDTH_M: f32 = 0.01;
const MAX_CORRECTION_CFL: f32 = 1e-5;

fn finite(value: f32) -> bool {
    return value == value && value <= 3.402823e38 && value >= -3.402823e38;
}

// Each parity pass owns disjoint cell pairs. The shared-face adjustment
// diffuses local volume residuals without changing their component total.
fn pair(id: u32, axis: u32, parity: u32) {
    let face_count = select((params.width + 1u) * params.height,
                            params.width * (params.height + 1u), axis == 1u);
    if id >= face_count {
        return;
    }
    let stride = select(params.width + 1u, params.width, axis == 1u);
    let x = id % stride;
    let y = id / stride;
    if axis == 0u {
        if x == 0u || x == params.width || x % 2u != parity {
            return;
        }
    } else if y == 0u || y == params.height || y % 2u != parity {
        return;
    }
    let negative_cell = select((y - 1u) * params.width + x,
                               y * params.width + x - 1u, axis == 0u);
    let positive_cell = y * params.width + x;
    if headers[negative_cell].fixed_wall != 0u || headers[positive_cell].fixed_wall != 0u {
        return;
    }
    var open = 0.0;
    var previous = 0.0;
    if axis == 0u {
        open = u_aperture[id];
        previous = u_velocity[id];
    } else {
        open = v_aperture[id];
        previous = v_velocity[id];
    }
    if open <= 0.0 {
        return;
    }
    let negative = residual[negative_cell];
    let positive = residual[positive_cell];
    let delta = 0.25 * (negative - positive);
    if delta == 0.0 {
        return;
    }
    let correction_cfl = delta / open;
    let corrected = previous + correction_cfl * CELL_WIDTH_M / params.dt_s;
    if !(finite(corrected) && finite(correction_cfl)
         && abs(correction_cfl) <= MAX_CORRECTION_CFL) {
        atomicOr(&status[0], CORRECTION_LIMIT);
        return;
    }
    let actual = ((corrected - previous) * params.dt_s / CELL_WIDTH_M) * open;
    if axis == 0u {
        u_velocity[id] = corrected;
    } else {
        v_velocity[id] = corrected;
    }
    residual[negative_cell] = negative - actual;
    residual[positive_cell] = positive + actual;
}

@compute @workgroup_size(64)
fn pair_faces(@builtin(global_invocation_id) id: vec3<u32>) {
    pair(id.x, params.reserved & 1u, (params.reserved >> 1u) & 1u);
}
