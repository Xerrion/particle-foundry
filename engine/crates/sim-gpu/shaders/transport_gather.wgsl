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

struct FaceFlux {
    volume: f32,
    liquid_mass: f32,
    carrier_mass: f32,
    liquid_marker: f32,
    carrier_marker: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_mass: array<f32>;
@group(0) @binding(2) var<storage, read> input_marker: array<f32>;
@group(0) @binding(3) var<storage, read> headers: array<CellHeader>;
@group(0) @binding(4) var<storage, read> fluxes: array<FaceFlux>;
@group(0) @binding(5) var<storage, read_write> output_mass: array<f32>;
@group(0) @binding(6) var<storage, read_write> output_marker: array<f32>;
@group(0) @binding(7) var<storage, read_write> status: array<atomic<u32>>;

const INVALID_CELL: u32 = 2u;
const DONOR_OVERDRAW: u32 = 4u;
const VOLUME_CLOSURE: u32 = 8u;

fn finite(value: f32) -> bool {
    return value == value && value <= 3.402823e38 && value >= -3.402823e38;
}

fn gather_amount(owned: f32, negative_face: f32, positive_face: f32) -> f32 {
    let outgoing = max(0.0, -negative_face) + max(0.0, positive_face);
    let incoming = max(0.0, negative_face) + max(0.0, -positive_face);
    if !(finite(owned) && owned >= 0.0 && finite(outgoing) && finite(incoming)) {
        atomicOr(&status[0], INVALID_CELL);
        return 0.0;
    }
    if outgoing > owned {
        atomicOr(&status[0], DONOR_OVERDRAW);
        return 0.0;
    }
    // Explicit fma boundaries keep a small incoming amount after exhaustion.
    // Reassociation of a plain subtraction/addition can erase that amount.
    var result = fma(1.0, incoming, fma(-1.0, outgoing, owned));
    if outgoing <= 0.5 * owned {
        result = fma(1.0, owned, fma(-1.0, outgoing, incoming));
    }
    if !(finite(result) && result >= 0.0) {
        atomicOr(&status[0], INVALID_CELL);
        return 0.0;
    }
    return result;
}

fn gather(cell: u32, axis: u32) {
    if cell >= params.cells {
        return;
    }
    let x = cell % params.width;
    let y = cell / params.width;
    let negative_face = select(y * params.width + x,
                               y * (params.width + 1u) + x, axis == 0u);
    let positive_face = select((y + 1u) * params.width + x,
                               negative_face + 1u, axis == 0u);
    let negative = fluxes[negative_face];
    let positive = fluxes[positive_face];
    let liquid = gather_amount(input_mass[cell], negative.liquid_mass, positive.liquid_mass);
    let carrier = gather_amount(input_mass[params.cells + cell],
                                negative.carrier_mass, positive.carrier_mass);
    let liquid_marker = gather_amount(input_marker[cell],
                                      negative.liquid_marker, positive.liquid_marker);
    let carrier_marker = gather_amount(input_marker[params.cells + cell],
                                       negative.carrier_marker, positive.carrier_marker);
    if headers[cell].fixed_wall != 0u {
        if liquid != 0.0 || carrier != 0.0 || liquid_marker != 0.0 || carrier_marker != 0.0 {
            atomicOr(&status[0], INVALID_CELL);
        }
    }
    if (liquid == 0.0 && liquid_marker != 0.0)
       || (carrier == 0.0 && carrier_marker != 0.0) {
        atomicOr(&status[0], INVALID_CELL);
    }
    if axis == 1u && headers[cell].fixed_wall == 0u {
        let volume = liquid / params.liquid_density + carrier / params.carrier_density;
        if !(finite(volume) && abs(volume - params.cell_volume) <= 1e-5 * params.cell_volume) {
            atomicOr(&status[0], VOLUME_CLOSURE);
        }
    }
    output_mass[cell] = liquid;
    output_mass[params.cells + cell] = carrier;
    output_marker[cell] = liquid_marker;
    output_marker[params.cells + cell] = carrier_marker;
}

@compute @workgroup_size(64)
fn x_cells(@builtin(global_invocation_id) global_id: vec3<u32>) {
    gather(global_id.x, 0u);
}

@compute @workgroup_size(64)
fn y_cells(@builtin(global_invocation_id) global_id: vec3<u32>) {
    gather(global_id.x, 1u);
}
