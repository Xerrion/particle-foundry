// A requested cell in the selected committed generation. The host submits
// exactly one workgroup and reads back four scalar words, not a field plane.
struct ProbeParams {
    cell_count: u32,
    cell_index: u32,
    reserved0: u32,
    reserved1: u32,
}

struct CellHeader {
    energy_j: f32,
    fixed_wall: u32,
}

@group(0) @binding(0) var<uniform> params: ProbeParams;
@group(0) @binding(1) var<storage, read> headers: array<CellHeader>;
@group(0) @binding(2) var<storage, read> mass_kg: array<f32>;
@group(0) @binding(3) var<storage, read_write> result: array<u32>;

@compute @workgroup_size(1)
fn main() {
    let cell = params.cell_index;
    result[0] = bitcast<u32>(mass_kg[cell]);
    result[1] = bitcast<u32>(mass_kg[params.cell_count + cell]);
    result[2] = headers[cell].fixed_wall;
    result[3] = 0u;
}
