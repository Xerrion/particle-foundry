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
@group(0) @binding(1) var<storage, read> masses: array<f32>;
@group(0) @binding(2) var<storage, read> headers: array<CellHeader>;
@group(0) @binding(3) var<storage, read> u_velocity: array<f32>;
@group(0) @binding(4) var<storage, read> v_velocity: array<f32>;
@group(0) @binding(5) var<storage, read> u_aperture: array<f32>;
@group(0) @binding(6) var<storage, read> v_aperture: array<f32>;
@group(0) @binding(7) var<storage, read_write> residual: array<f32>;
@group(0) @binding(8) var<storage, read_write> status: array<atomic<u32>>;

const INVALID_CELL: u32 = 2u;
const CELL_WIDTH_M: f32 = 0.01;
const PACKED_SOURCE_DEADBAND: f32 = 1e-6;

fn finite(value: f32) -> bool {
    return value == value && value <= 3.402823e38 && value >= -3.402823e38;
}

fn raw_flux(speed: f32, open: f32) -> f32 {
    return (speed * params.dt_s / CELL_WIDTH_M) * open;
}

// Keep the initial f32 mass-packing error as a small local allowance. A
// conservative face correction cannot remove a uniform global packing offset.
@compute @workgroup_size(64)
fn cell_residual(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let cell = global_id.x;
    if cell >= params.cells {
        return;
    }
    residual[cell] = 0.0;
    if headers[cell].fixed_wall != 0u {
        return;
    }
    let liquid_volume = masses[cell] / params.liquid_density;
    let carrier_volume = masses[params.cells + cell] / params.carrier_density;
    let fill = (liquid_volume + carrier_volume) / params.cell_volume;
    if !(finite(fill) && fill > 0.0) {
        atomicOr(&status[0], INVALID_CELL);
        return;
    }
    let x = cell % params.width;
    let y = cell / params.width;
    let left_face = y * (params.width + 1u) + x;
    let top_face = cell;
    let divergence = raw_flux(u_velocity[left_face + 1u], u_aperture[left_face + 1u])
                   - raw_flux(u_velocity[left_face], u_aperture[left_face])
                   + raw_flux(v_velocity[top_face + params.width],
                              v_aperture[top_face + params.width])
                   - raw_flux(v_velocity[top_face], v_aperture[top_face]);
    let source_error = fill - 1.0;
    let excess_source_error = select(0.0, source_error - sign(source_error)
                                     * PACKED_SOURCE_DEADBAND,
                                     abs(source_error) > PACKED_SOURCE_DEADBAND);
    let value = excess_source_error - divergence;
    if !finite(value) {
        atomicOr(&status[0], INVALID_CELL);
        return;
    }
    residual[cell] = value;
}
