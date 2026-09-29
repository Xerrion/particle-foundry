struct GridParams {
    width: u32,
    height: u32,
    components: u32,
    reserved: u32,
}

@group(0) @binding(0) var<uniform> grid: GridParams;
@group(0) @binding(1) var<storage, read> u_velocity: array<f32>;
@group(0) @binding(2) var<storage, read> v_velocity: array<f32>;
// Positive finite f32 values have monotonically ordered u32 bit patterns.
// Word zero owns the maximum rate bits. Word one owns the error flags.
@group(0) @binding(3) var<storage, read_write> result: array<atomic<u32>>;

var<workgroup> lane_rate_bits: array<u32, 64>;

const INVALID_SPEED: u32 = 1u;
const INVALID_RATE: u32 = 2u;

fn valid_speed(value: f32) -> bool {
    let magnitude = bitcast<u32>(value) & 0x7fffffffu;
    // Reject NaN, infinity, and nonzero subnormals that a GPU may flush.
    return magnitude < 0x7f800000u
        && (magnitude == 0u || magnitude >= 0x00800000u);
}

fn finite(value: f32) -> bool {
    return value == value && value <= 3.402823e38 && value >= -3.402823e38;
}

@compute @workgroup_size(64)
fn reduce_rate(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    let cell = global_id.x;
    let lane = local_id.x;
    var rate_bits = 0u;
    let cells = grid.width * grid.height;
    if cell < cells {
        let x = cell % grid.width;
        let y = cell / grid.width;
        let u_left = u_velocity[y * (grid.width + 1u) + x];
        let u_right = u_velocity[y * (grid.width + 1u) + x + 1u];
        let v_top = v_velocity[y * grid.width + x];
        let v_bottom = v_velocity[(y + 1u) * grid.width + x];
        if !(valid_speed(u_left) && valid_speed(u_right)
             && valid_speed(v_top) && valid_speed(v_bottom)) {
            atomicOr(&result[1], INVALID_SPEED);
        } else {
            let incoming = max(u_left, 0.0) + max(-u_right, 0.0)
                         + max(v_top, 0.0) + max(-v_bottom, 0.0);
            let outgoing = max(-u_left, 0.0) + max(u_right, 0.0)
                         + max(-v_top, 0.0) + max(v_bottom, 0.0);
            let rate = max(incoming, outgoing);
            if !finite(rate) {
                atomicOr(&result[1], INVALID_RATE);
            } else {
                // A negative-zero bit pattern would outrank every positive
                // f32 rate in the unsigned atomic maximum.
                rate_bits = bitcast<u32>(rate) & 0x7fffffffu;
            }
        }
    }
    lane_rate_bits[lane] = rate_bits;
    workgroupBarrier();

    var offset = 32u;
    loop {
        if lane < offset {
            lane_rate_bits[lane] = max(lane_rate_bits[lane], lane_rate_bits[lane + offset]);
        }
        workgroupBarrier();
        if offset == 1u { break; }
        offset = offset / 2u;
    }
    if lane == 0u {
        atomicMax(&result[0], lane_rate_bits[0]);
    }
}
