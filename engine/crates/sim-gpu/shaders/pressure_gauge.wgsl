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
@group(0) @binding(1) var<storage, read> root_cell: array<u32>;
@group(0) @binding(2) var<storage, read_write> heads: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> next_cell: array<u32>;
@group(0) @binding(4) var<storage, read_write> pressure: array<f32>;
@group(0) @binding(5) var<storage, read_write> component_mean: array<f32>;

// Each cell enters exactly one component list. The list avoids a full-grid
// scan for each disconnected component and needs no floating-point atomics.
@compute @workgroup_size(64)
fn reset(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x < params.cells) {
        atomicStore(&heads[id.x], 0xffffffffu);
    }
}

@compute @workgroup_size(64)
fn link(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell < params.cells) {
        let root = root_cell[cell];
        next_cell[cell] = atomicExchange(&heads[root], cell);
    }
}

@compute @workgroup_size(64)
fn mean(@builtin(global_invocation_id) id: vec3<u32>) {
    let root = id.x;
    if (root >= params.cells || root_cell[root] != root) {
        return;
    }

    var sum = 0.0;
    var compensation = 0.0;
    var count = 0u;
    var cell = atomicLoad(&heads[root]);
    // Valid component labels give acyclic lists. The bound keeps this dispatch
    // finite even if the stage boundary supplies invalid labels.
    loop {
        if (cell == 0xffffffffu || count == params.cells) {
            break;
        }
        let value = pressure[cell];
        let updated = sum + value;
        if (abs(sum) >= abs(value)) {
            compensation += (sum - updated) + value;
        } else {
            compensation += (value - updated) + sum;
        }
        sum = updated;
        count += 1u;
        cell = next_cell[cell];
    }
    component_mean[root] = (sum + compensation) / f32(count);
}

@compute @workgroup_size(64)
fn center(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell < params.cells) {
        pressure[cell] -= component_mean[root_cell[cell]];
    }
}
