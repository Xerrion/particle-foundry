struct Params {
    cells: u32,
    cell_volume_m3: f32,
    liquid_density_kg_m3: f32,
    carrier_density_kg_m3: f32,
}

struct CellHeader {
    energy_j: f32,
    fixed_wall: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> mass_kg: array<f32>;
@group(0) @binding(2) var<storage, read> headers: array<CellHeader>;
@group(0) @binding(3) var<storage, read_write> density_kg_m3: array<f32>;
@group(0) @binding(4) var<storage, read_write> status: array<atomic<u32>>;

const INVALID_CELL: u32 = 1u;
const VOLUME_CLOSURE: u32 = 2u;
const VOLUME_CLOSURE_REL_TOL: f32 = 1e-5;

fn finite(value: f32) -> bool {
    return (bitcast<u32>(value) & 0x7f800000u) != 0x7f800000u;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell >= params.cells) {
        return;
    }

    let liquid = mass_kg[cell];
    let carrier = mass_kg[params.cells + cell];
    let wall = headers[cell].fixed_wall;
    if (wall == 1u) {
        density_kg_m3[cell] = params.carrier_density_kg_m3;
        if (!finite(liquid) || !finite(carrier) || liquid != 0.0 || carrier != 0.0) {
            atomicOr(&status[0], INVALID_CELL);
        }
        return;
    }
    if (!finite(liquid) || !finite(carrier) || liquid < 0.0 || carrier < 0.0 || wall != 0u) {
        density_kg_m3[cell] = 0.0;
        atomicOr(&status[0], INVALID_CELL);
        return;
    }

    let liquid_volume = liquid / params.liquid_density_kg_m3;
    let carrier_volume = carrier / params.carrier_density_kg_m3;
    let volume = liquid_volume + carrier_volume;
    let derived_density = (liquid + carrier) / params.cell_volume_m3;
    // Validate tiny owned phases in cell units before density division.
    // A normal mass can have a physical volume below the f32 normal range.
    let liquid_fill = (liquid / params.cell_volume_m3) / params.liquid_density_kg_m3;
    let carrier_fill = (carrier / params.cell_volume_m3) / params.carrier_density_kg_m3;
    if (!finite(liquid_volume) || !finite(carrier_volume) || !finite(volume)
        || !finite(derived_density) || derived_density <= 0.0
        || !finite(liquid_fill) || !finite(carrier_fill)
        || (liquid > 0.0 && liquid_fill == 0.0)
        || (carrier > 0.0 && carrier_fill == 0.0)) {
        density_kg_m3[cell] = 0.0;
        atomicOr(&status[0], INVALID_CELL);
        return;
    }
    if (abs(volume - params.cell_volume_m3)
            > VOLUME_CLOSURE_REL_TOL * params.cell_volume_m3
        || liquid_volume > params.cell_volume_m3 * (1.0 + VOLUME_CLOSURE_REL_TOL)) {
        density_kg_m3[cell] = 0.0;
        atomicOr(&status[0], VOLUME_CLOSURE);
        return;
    }
    density_kg_m3[cell] = derived_density;
}
