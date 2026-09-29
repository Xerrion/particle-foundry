struct PaintParams {
    width: u32,
    height: u32,
    center_x: u32,
    center_y: u32,
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

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    let cells = params.width * params.height;
    if (cell >= cells) {
        return;
    }

    let x = cell % params.width;
    let y = cell / params.width;
    let dx = i32(x) - i32(params.center_x);
    let dy = i32(y) - i32(params.center_y);
    let radius = i32(params.radius);
    let selected = headers[cell].fixed_wall == 0u &&
        dx * dx + dy * dy <= radius * radius;

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
