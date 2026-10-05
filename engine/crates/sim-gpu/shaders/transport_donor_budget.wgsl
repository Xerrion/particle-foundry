struct Params {
    width: u32,
    height: u32,
    cells: u32,
    axis: u32,
    dt_s: f32,
    liquid_density: f32,
    carrier_density: f32,
    cell_volume: f32,
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
@group(0) @binding(3) var<storage, read> provisional: array<FaceFlux>;
@group(0) @binding(4) var<storage, read_write> corrected: array<FaceFlux>;
@group(0) @binding(5) var<storage, read_write> status: array<atomic<u32>>;

const INVALID_CELL: u32 = 2u;
const DONOR_OVERDRAW: u32 = 4u;

fn finite(value: f32) -> bool {
    return value == value && value <= 3.402823e38 && value >= -3.402823e38;
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

// Gather adds both outgoing masses before subtracting their shared inventory.
// Use that same rounded sum for the marker budget, including full exhaustion.
fn marker_budget(owned_marker: f32, owned_mass: f32,
                 negative_mass: f32, positive_mass: f32) -> vec2<f32> {
    let total = marker_flux(owned_marker, owned_mass, negative_mass + positive_mass);
    let provisional_negative = min(total, marker_flux(owned_marker, owned_mass, negative_mass));
    let positive = total - provisional_negative;
    // Recompute the first share from the complement so the two f32 faces use
    // the same total even when their magnitudes differ by many orders.
    let negative = total - positive;
    return vec2<f32>(negative, positive);
}

// Both output-face threads calculate the same split from an immutable ledger.
fn negative_share(owned: f32, negative: f32, positive: f32) -> f32 {
    let scale = max(negative, positive);
    let negative_fraction = negative / scale;
    let positive_fraction = positive / scale;
    let split = negative_fraction / (negative_fraction + positive_fraction);
    return min(negative, owned * split);
}

@compute @workgroup_size(64)
fn budget_face(@builtin(global_invocation_id) id: vec3<u32>) {
    let face = id.x;
    let face_count = select((params.width + 1u) * params.height,
                            params.width * (params.height + 1u), params.axis == 1u);
    if face >= face_count {
        return;
    }
    let raw = provisional[face];
    corrected[face] = raw;
    if raw.volume == 0.0 {
        return;
    }
    let stride = select(params.width + 1u, params.width, params.axis == 1u);
    let x = face % stride;
    let y = face / stride;
    let negative_cell = select((y - 1u) * params.width + x,
                               y * params.width + x - 1u, params.axis == 0u);
    let positive_cell = y * params.width + x;
    let donor = select(positive_cell, negative_cell, raw.volume > 0.0);
    let donor_x = donor % params.width;
    let donor_y = donor / params.width;
    let negative_face = select(donor_y * params.width + donor_x,
                               donor_y * (params.width + 1u) + donor_x, params.axis == 0u);
    let positive_face = select(negative_face + params.width,
                               negative_face + 1u, params.axis == 0u);
    let negative_flux = provisional[negative_face];
    let positive_flux = provisional[positive_face];
    let negative_liquid = max(0.0, -negative_flux.liquid_mass);
    let positive_liquid = max(0.0, positive_flux.liquid_mass);
    let negative_carrier = max(0.0, -negative_flux.carrier_mass);
    let positive_carrier = max(0.0, positive_flux.carrier_mass);
    let owned_liquid = masses[donor];
    let owned_carrier = masses[params.cells + donor];
    let owned_liquid_marker = markers[donor];
    let owned_carrier_marker = markers[params.cells + donor];
    if !(finite(owned_liquid) && finite(owned_carrier)
         && finite(owned_liquid_marker) && finite(owned_carrier_marker)
         && owned_liquid >= 0.0 && owned_carrier >= 0.0
         && owned_liquid_marker >= 0.0 && owned_carrier_marker >= 0.0) {
        atomicOr(&status[0], INVALID_CELL);
        return;
    }
    let liquid_overdraw = negative_liquid > owned_liquid - positive_liquid;
    let carrier_overdraw = negative_carrier > owned_carrier - positive_carrier;
    if liquid_overdraw && carrier_overdraw {
        atomicOr(&status[0], DONOR_OVERDRAW);
        return;
    }
    let negative_volume = max(0.0, -negative_flux.volume);
    let positive_volume = max(0.0, positive_flux.volume);
    let is_negative = face == negative_face;
    var negative_liquid_mass = negative_liquid;
    var positive_liquid_mass = positive_liquid;
    var negative_carrier_mass = negative_carrier;
    var positive_carrier_mass = positive_carrier;
    if liquid_overdraw {
        negative_liquid_mass = negative_share(owned_liquid, negative_liquid, positive_liquid);
        positive_liquid_mass = min(positive_liquid, owned_liquid - negative_liquid_mass);
        negative_carrier_mass = max(0.0, negative_volume
                                    - negative_liquid_mass / params.liquid_density)
                                * params.carrier_density;
        positive_carrier_mass = max(0.0, positive_volume
                                    - positive_liquid_mass / params.liquid_density)
                                * params.carrier_density;
    } else if carrier_overdraw {
        negative_carrier_mass = negative_share(owned_carrier, negative_carrier, positive_carrier);
        positive_carrier_mass = min(positive_carrier, owned_carrier - negative_carrier_mass);
        negative_liquid_mass = max(0.0, negative_volume
                                   - negative_carrier_mass / params.carrier_density)
                               * params.liquid_density;
        positive_liquid_mass = max(0.0, positive_volume
                                   - positive_carrier_mass / params.carrier_density)
                               * params.liquid_density;
    }
    if !(finite(negative_liquid_mass) && finite(positive_liquid_mass)
         && finite(negative_carrier_mass) && finite(positive_carrier_mass)
         && negative_liquid_mass >= 0.0 && positive_liquid_mass >= 0.0
         && negative_carrier_mass >= 0.0 && positive_carrier_mass >= 0.0)
       || negative_liquid_mass > owned_liquid - positive_liquid_mass
       || negative_carrier_mass > owned_carrier - positive_carrier_mass {
        atomicOr(&status[0], DONOR_OVERDRAW);
        return;
    }
    let liquid_mass = select(positive_liquid_mass, negative_liquid_mass, is_negative);
    let carrier_mass = select(positive_carrier_mass, negative_carrier_mass, is_negative);
    let liquid_marker = marker_budget(owned_liquid_marker, owned_liquid,
                                      negative_liquid_mass, positive_liquid_mass);
    let carrier_marker = marker_budget(owned_carrier_marker, owned_carrier,
                                       negative_carrier_mass, positive_carrier_mass);
    let sign = select(1.0, -1.0, is_negative);
    corrected[face] = FaceFlux(raw.volume,
                               sign * liquid_mass,
                               sign * carrier_mass,
                               sign * select(liquid_marker.y, liquid_marker.x, is_negative),
                               sign * select(carrier_marker.y, carrier_marker.x, is_negative));
}
