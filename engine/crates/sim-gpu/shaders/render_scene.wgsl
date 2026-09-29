// Direct M3 scene display. The first mass plane is liquid, the second carrier.
// Cell headers use the portable eight-byte energy/fixed-wall layout.
struct RenderParams {
    grid_target: vec4<u32>,
    visible_rect: vec4<f32>,
    phase_density_volume: vec4<f32>,
}

struct CellHeader {
    energy_j: f32,
    fixed_wall: u32,
}

@group(0) @binding(0) var<uniform> params: RenderParams;
@group(0) @binding(1) var<storage, read> headers: array<CellHeader>;
@group(0) @binding(2) var<storage, read> mass_kg: array<f32>;

@vertex
fn vertex_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    let xy = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(xy[vertex_index], 0.0, 1.0);
}

@fragment
fn fragment_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = position.xy / vec2<f32>(params.grid_target.zw);
    let world = params.visible_rect.xy + uv * params.visible_rect.zw;
    if (world.x < 0.0 || world.y < 0.0 ||
        world.x >= f32(params.grid_target.x) || world.y >= f32(params.grid_target.y)) {
        return vec4<f32>(4.0, 8.0, 12.0, 255.0) / 255.0;
    }

    let cell = u32(floor(world.y)) * params.grid_target.x + u32(floor(world.x));
    if (headers[cell].fixed_wall != 0u) {
        return vec4<f32>(89.0, 94.0, 99.0, 255.0) / 255.0;
    }

    let cell_count = params.grid_target.x * params.grid_target.y;
    let liquid_fraction = clamp(
        mass_kg[cell] / (params.phase_density_volume.x * params.phase_density_volume.z),
        0.0,
        1.0,
    );
    let carrier_fraction = clamp(
        mass_kg[cell_count + cell] /
            (params.phase_density_volume.y * params.phase_density_volume.z),
        0.0,
        1.0,
    );
    let empty = vec3<f32>(4.0, 8.0, 12.0) / 255.0;
    let air = vec3<f32>(8.0, 18.0, 27.0) / 255.0;
    let water = vec3<f32>(35.0, 116.0, 203.0) / 255.0;
    let displayed_carrier = min(carrier_fraction, 1.0 - liquid_fraction);
    let empty_fraction = 1.0 - liquid_fraction - displayed_carrier;
    let color = empty * empty_fraction + air * displayed_carrier + water * liquid_fraction;
    return vec4<f32>(color, 1.0);
}
