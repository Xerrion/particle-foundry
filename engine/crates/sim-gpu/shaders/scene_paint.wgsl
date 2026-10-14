struct PaintParams {
    width: u32,
    height: u32,
    from_x: u32,
    from_y: u32,
    to_x: u32,
    to_y: u32,
    radius: u32,
    phase: u32,
    selected_mass_kg: f32,
    selected_density_kg_m3: f32,
}

struct CellHeader {
    energy_j: f32,
    fixed_wall: u32,
}

@group(0) @binding(0) var<uniform> params: PaintParams;
@group(0) @binding(1) var<storage, read> headers: array<CellHeader>;
@group(0) @binding(2) var<storage, read> committed_mass: array<f32>;
@group(0) @binding(3) var<storage, read> committed_marker: array<f32>;
@group(0) @binding(4) var<storage, read> committed_density: array<f32>;
@group(0) @binding(5) var<storage, read_write> candidate_mass: array<f32>;
@group(0) @binding(6) var<storage, read_write> candidate_marker: array<f32>;
@group(0) @binding(7) var<storage, read_write> candidate_density: array<f32>;

fn inside_stroke(x: u32, y: u32) -> bool {
    let dx = i32(x) - i32(params.from_x);
    let dy = i32(y) - i32(params.from_y);
    let ex = i32(x) - i32(params.to_x);
    let ey = i32(y) - i32(params.to_y);
    let radius_squared = i32(params.radius * params.radius);
    // Endpoint disks retain the exact integer classification of point painting.
    if (dx * dx + dy * dy <= radius_squared ||
        ex * ex + ey * ey <= radius_squared) {
        return true;
    }
    let sx = i32(params.to_x) - i32(params.from_x);
    let sy = i32(params.to_y) - i32(params.from_y);
    let cross_product = dx * sy - dy * sx;
    if (params.radius == 0u) {
        // A thin stroke selects cell interiors crossed by the segment. Strict
        // corner ties keep diagonal strokes one cell wide in either direction.
        return x >= min(params.from_x, params.to_x) &&
            x <= max(params.from_x, params.to_x) &&
            y >= min(params.from_y, params.to_y) &&
            y <= max(params.from_y, params.to_y) &&
            2 * abs(cross_product) < abs(sx) + abs(sy);
    }
    let length_squared = sx * sx + sy * sy;
    let projection = dx * sx + dy * sy;
    if (projection <= 0 || projection >= length_squared) {
        return false;
    }
    // Integer products make reversed endpoints symmetric. Convert before squaring
    // the cross product because its square can exceed the i32 range.
    let cross = f32(cross_product);
    return cross * cross <= f32(radius_squared) * f32(length_squared);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    let cells = params.width * params.height;
    if (cell >= cells) {
        return;
    }

    let x = cell % params.width;
    let y = cell / params.width;
    let selected = headers[cell].fixed_wall == 0u && inside_stroke(x, y);

    if (selected) {
        candidate_mass[cell] = select(params.selected_mass_kg, 0.0, params.phase == 1u);
        candidate_mass[cells + cell] = select(0.0, params.selected_mass_kg, params.phase == 1u);
        candidate_marker[cell] = 0.0;
        candidate_marker[cells + cell] = 0.0;
        candidate_density[cell] = params.selected_density_kg_m3;
    } else {
        candidate_mass[cell] = committed_mass[cell];
        candidate_mass[cells + cell] = committed_mass[cells + cell];
        candidate_marker[cell] = committed_marker[cell];
        candidate_marker[cells + cell] = committed_marker[cells + cell];
        candidate_density[cell] = committed_density[cell];
    }
}
