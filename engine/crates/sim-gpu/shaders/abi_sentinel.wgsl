struct GridParams {
    width: u32,
    height: u32,
    active_count: u32,
    reserved: u32,
}

struct CellHeader {
    energy_j: f32,
    fixed_wall: u32,
}

@group(0) @binding(0) var<uniform> grid: GridParams;
@group(0) @binding(1) var<storage, read> cells: array<CellHeader>;
@group(0) @binding(2) var<storage, read> mass_kg: array<f32>;
@group(0) @binding(3) var<storage, read_write> result: array<u32>;

@compute @workgroup_size(1)
fn main() {
    let last_cell = grid.width * grid.height - 1u;
    let last_slot = grid.active_count - 1u;
    let last_mass = last_slot * grid.width * grid.height + last_cell;

    result[0] = grid.width;
    result[1] = grid.height;
    result[2] = grid.active_count;
    result[3] = grid.reserved;
    result[4] = bitcast<u32>(cells[0].energy_j);
    result[5] = cells[0].fixed_wall;
    result[6] = bitcast<u32>(cells[last_cell].energy_j);
    result[7] = cells[last_cell].fixed_wall;
    result[8] = bitcast<u32>(mass_kg[0]);
    result[9] = bitcast<u32>(mass_kg[last_cell]);
    result[10] = bitcast<u32>(mass_kg[last_slot * grid.width * grid.height]);
    result[11] = bitcast<u32>(mass_kg[last_mass]);
}
