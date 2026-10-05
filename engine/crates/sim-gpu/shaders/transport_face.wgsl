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
@group(0) @binding(1) var<storage, read> masses: array<f32>;
@group(0) @binding(2) var<storage, read> markers: array<f32>;
@group(0) @binding(3) var<storage, read> headers: array<CellHeader>;
@group(0) @binding(4) var<storage, read> velocity: array<f32>;
@group(0) @binding(5) var<storage, read> aperture: array<f32>;
@group(0) @binding(6) var<storage, read_write> fluxes: array<FaceFlux>;
@group(0) @binding(7) var<storage, read_write> status: array<atomic<u32>>;

const INVALID_FACE: u32 = 1u;
const INVALID_CELL: u32 = 2u;
const MAX_FACE_CFL: f32 = 0.5;
const CELL_WIDTH_M: f32 = 0.01;

fn finite(value: f32) -> bool {
    return value == value && value <= 3.402823e38 && value >= -3.402823e38;
}

fn zero_flux() -> FaceFlux {
    return FaceFlux(0.0, 0.0, 0.0, 0.0, 0.0);
}

fn phase_fraction(cell: u32, fallback: f32) -> f32 {
    if headers[cell].fixed_wall != 0u {
        return fallback;
    }
    let liquid_volume = masses[cell] / params.liquid_density;
    let carrier_volume = masses[params.cells + cell] / params.carrier_density;
    let total = liquid_volume + carrier_volume;
    if !(finite(total) && total > 0.0) {
        return fallback;
    }
    return liquid_volume / total;
}

fn bounded_phase_flux(owned_mass: f32, owned_volume: f32, swept_volume: f32) -> f32 {
    if swept_volume <= 0.0 {
        return 0.0;
    }
    if swept_volume >= owned_volume {
        return owned_mass;
    }
    let swept_fraction = clamp(swept_volume / owned_volume, 0.0, 1.0);
    return min(owned_mass, owned_mass * swept_fraction);
}

fn marker_flux(owned_marker: f32, owned_mass: f32, moved_mass: f32) -> f32 {
    if moved_mass == 0.0 || owned_mass == 0.0 {
        return 0.0;
    }
    if moved_mass >= owned_mass {
        return owned_marker;
    }
    let moved_fraction = clamp(moved_mass / owned_mass, 0.0, 1.0);
    return min(owned_marker, owned_marker * moved_fraction);
}

fn face(id: u32, axis: u32) {
    let face_count = select((params.width + 1u) * params.height,
                            params.width * (params.height + 1u), axis == 1u);
    if id >= face_count {
        return;
    }
    fluxes[id] = zero_flux();
    let stride = select(params.width + 1u, params.width, axis == 1u);
    let x = id % stride;
    let y = id / stride;
    let outer = select(x == 0u || x == params.width,
                       y == 0u || y == params.height, axis == 1u);
    let speed = velocity[id];
    let open = aperture[id];
    if !(finite(speed) && finite(open) && open >= 0.0 && open <= 1.0) {
        atomicOr(&status[0], INVALID_FACE);
        return;
    }
    if outer {
        if speed != 0.0 || open != 0.0 {
            atomicOr(&status[0], INVALID_FACE);
        }
        return;
    }
    let negative_cell = select((y - 1u) * params.width + x,
                               y * params.width + x - 1u, axis == 0u);
    let positive_cell = select(y * params.width + x,
                               y * params.width + x, axis == 0u);
    if headers[negative_cell].fixed_wall != 0u || headers[positive_cell].fixed_wall != 0u {
        if speed != 0.0 || open != 0.0 {
            atomicOr(&status[0], INVALID_FACE);
        }
        return;
    }
    if open == 0.0 {
        if speed != 0.0 {
            atomicOr(&status[0], INVALID_FACE);
        }
        return;
    }
    if speed == 0.0 {
        return;
    }
    let cfl = abs(speed) * params.dt_s / CELL_WIDTH_M;
    if !(finite(cfl) && cfl >= 0.0 && cfl <= MAX_FACE_CFL) {
        atomicOr(&status[0], INVALID_FACE);
        return;
    }
    let donor = select(positive_cell, negative_cell, speed > 0.0);
    let liquid_mass = masses[donor];
    let carrier_mass = masses[params.cells + donor];
    let liquid_marker = markers[donor];
    let carrier_marker = markers[params.cells + donor];
    if !(finite(liquid_mass) && finite(carrier_mass) && finite(liquid_marker)
         && finite(carrier_marker) && liquid_mass >= 0.0 && carrier_mass >= 0.0
         && liquid_marker >= 0.0 && carrier_marker >= 0.0) {
        atomicOr(&status[0], INVALID_CELL);
        return;
    }
    let liquid_volume = liquid_mass / params.liquid_density;
    let carrier_volume = carrier_mass / params.carrier_density;
    let donor_volume = liquid_volume + carrier_volume;
    let swept_volume = cfl * open * params.cell_volume;
    if !(finite(donor_volume) && donor_volume > 0.0 && finite(swept_volume)
         && swept_volume <= donor_volume) {
        atomicOr(&status[0], INVALID_CELL);
        return;
    }

    let alpha = liquid_volume / donor_volume;
    let donor_x = donor % params.width;
    let donor_y = donor / params.width;
    var before = donor;
    var after = donor;
    if axis == 0u {
        if donor_x > 0u { before = donor - 1u; }
        if donor_x + 1u < params.width { after = donor + 1u; }
    } else {
        if donor_y > 0u { before = donor - params.width; }
        if donor_y + 1u < params.height { after = donor + params.width; }
    }
    let gradient = phase_fraction(after, alpha) - phase_fraction(before, alpha);
    var transverse_before = donor;
    var transverse_after = donor;
    if axis == 0u {
        if donor_y > 0u { transverse_before = donor - params.width; }
        if donor_y + 1u < params.height { transverse_after = donor + params.width; }
    } else {
        if donor_x > 0u { transverse_before = donor - 1u; }
        if donor_x + 1u < params.width { transverse_after = donor + 1u; }
    }
    let transverse_gradient = phase_fraction(transverse_after, alpha) - phase_fraction(transverse_before, alpha);
    // Reconstruct one axis-aligned interface from the dominant gradient.
    // A tangent sweep crosses both phases in their owned proportions. Using
    // a separate interface direction for each sweep drains an entire tiny
    // phase through a tangent face and destabilizes a nearly flat free surface.
    // Ties select X, so both sweeps use the same reconstruction convention.
    let normal_sweep = select(abs(gradient) > abs(transverse_gradient),
                              abs(gradient) >= abs(transverse_gradient), axis == 0u);
    var liquid_swept = 0.0;
    // f32 already rounds unresolved fraction differences to zero. A resolved
    // gradient uses the CPU reconstruction rule. Rounding can change near ties.
    if alpha == 0.0 {
        liquid_swept = 0.0;
    } else if alpha == 1.0 {
        liquid_swept = swept_volume;
    } else if abs(gradient) <= 1e-12 || !normal_sweep {
        liquid_swept = swept_volume * alpha;
    } else if (gradient > 0.0) == (speed > 0.0) {
        liquid_swept = min(swept_volume, liquid_volume);
    } else {
        liquid_swept = max(0.0, swept_volume - carrier_volume);
    }
    liquid_swept = min(swept_volume, liquid_swept);
    let carrier_swept = swept_volume - liquid_swept;
    let liquid_flux = bounded_phase_flux(liquid_mass, liquid_volume, liquid_swept);
    let carrier_flux = bounded_phase_flux(carrier_mass, carrier_volume, carrier_swept);
    let liquid_marker_flux = marker_flux(liquid_marker, liquid_mass, liquid_flux);
    let carrier_marker_flux = marker_flux(carrier_marker, carrier_mass, carrier_flux);
    if !(finite(liquid_flux) && finite(carrier_flux)
         && finite(liquid_marker_flux) && finite(carrier_marker_flux)) {
        atomicOr(&status[0], INVALID_CELL);
        return;
    }
    let sign = select(-1.0, 1.0, speed > 0.0);
    fluxes[id] = FaceFlux(sign * swept_volume, sign * liquid_flux,
                          sign * carrier_flux, sign * liquid_marker_flux,
                          sign * carrier_marker_flux);
}

@compute @workgroup_size(64)
fn x_faces(@builtin(global_invocation_id) global_id: vec3<u32>) {
    face(global_id.x, 0u);
}

@compute @workgroup_size(64)
fn y_faces(@builtin(global_invocation_id) global_id: vec3<u32>) {
    face(global_id.x, 1u);
}
