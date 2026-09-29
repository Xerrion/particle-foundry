struct Params {
    width: u32,
    height: u32,
    cells: u32,
    partials: u32,
    dx_m: f32,
    dt_s: f32,
    residual_tolerance: f32,
    divergence_tolerance: f32,
    gravity_m_s2: f32,
    padding0: f32,
    padding1: f32,
    padding2: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> aperture_u: array<f32>;
@group(0) @binding(2) var<storage, read> aperture_v: array<f32>;
@group(0) @binding(3) var<storage, read> corrected_u: array<f32>;
@group(0) @binding(4) var<storage, read> corrected_v: array<f32>;
struct Partial {
    dot: f32,
    maximum: f32,
    invalid: u32,
    padding: u32,
}
@group(0) @binding(5) var<storage, read_write> partials: array<Partial>;

var<workgroup> maxima: array<f32, 64>;
var<workgroup> invalids: array<u32, 64>;

fn finite(value: f32) -> bool {
    return (bitcast<u32>(value) & 0x7f800000u) != 0x7f800000u;
}

@compute @workgroup_size(64)
fn main(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    let cell = id.x;
    var divergence = 0.0;
    if (cell < params.cells) {
        let x = cell % params.width;
        let y = cell / params.width;
        let left = y * (params.width + 1u) + x;
        let right = left + 1u;
        let top = y * params.width + x;
        let bottom = top + params.width;
        divergence = abs(
            aperture_u[right] * corrected_u[right]
            - aperture_u[left] * corrected_u[left]
            + aperture_v[bottom] * corrected_v[bottom]
            - aperture_v[top] * corrected_v[top]
        ) / params.dx_m;
    }
    invalids[lane] = u32(!finite(divergence));
    maxima[lane] = select(divergence, 0.0, invalids[lane] != 0u);
    workgroupBarrier();
    var span = 32u;
    loop {
        if (span == 0u) {
            break;
        }
        if (lane < span) {
            maxima[lane] = max(maxima[lane], maxima[lane + span]);
            invalids[lane] |= invalids[lane + span];
        }
        workgroupBarrier();
        span /= 2u;
    }
    if (lane == 0u) {
        partials[group.x] = Partial(0.0, maxima[0], invalids[0] | u32(!finite(maxima[0])), 0u);
    }
}
