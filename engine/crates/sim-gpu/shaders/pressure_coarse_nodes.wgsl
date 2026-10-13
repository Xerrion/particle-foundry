struct Params {
    previous_width: u32, previous_height: u32, width: u32, height: u32,
    dx_m: f32, damping: f32, padding0: f32, padding1: f32,
}
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> children: array<Node>;
@group(0) @binding(2) var<storage, read_write> parents: array<Node>;

@compute @workgroup_size(64)
fn roots(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell >= params.width * params.height) { return; }
    let x = 2u * (cell % params.width);
    let y = 2u * (cell / params.width);
    var root = 0xffffffffu;
    for (var dy = 0u; dy < 2u; dy += 1u) {
        for (var dx = 0u; dx < 2u; dx += 1u) {
            if (x + dx < params.previous_width && y + dy < params.previous_height) {
                root = min(root, children[(y + dy) * params.previous_width + x + dx].root);
            }
        }
    }
    var count = 0u;
    for (var dy = 0u; dy < 2u; dy += 1u) {
        for (var dx = 0u; dx < 2u; dx += 1u) {
            if (x + dx < params.previous_width && y + dy < params.previous_height) {
                let child = children[(y + dy) * params.previous_width + x + dx];
                if (child.root == root) { count += child.count; }
            }
        }
    }
    parents[cell].root = root;
    parents[cell].count = count;
}

@compute @workgroup_size(64)
fn weights(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell = id.x;
    if (cell >= params.width * params.height) { return; }
    let cx = cell % params.width;
    let cy = cell / params.width;
    let root = parents[cell].root;
    var boundary = vec4<f32>(0.0);
    var anchor = 0.0;
    for (var dy = 0u; dy < 2u; dy += 1u) {
        for (var dx = 0u; dx < 2u; dx += 1u) {
            let x = 2u * cx + dx;
            let y = 2u * cy + dy;
            if (x >= params.previous_width || y >= params.previous_height) { continue; }
            let child = children[y * params.previous_width + x];
            if (child.root != root || child.count == 0u) { continue; }
            let e = child.edges;
            anchor += max(child.diagonal - (e.x + e.y + e.z + e.w), 0.0);
            if (dx == 0u) { boundary.x += e.x; }
            if (dx == 1u || x + 1u == params.previous_width) { boundary.y += e.y; }
            if (dy == 0u) { boundary.z += e.z; }
            if (dy == 1u || y + 1u == params.previous_height) { boundary.w += e.w; }
        }
    }
    var edges = boundary;
    if (cx > 0u && parents[cell - 1u].root != root) { edges.x = 0.0; }
    if (cx + 1u < params.width && parents[cell + 1u].root != root) { edges.y = 0.0; }
    if (cy > 0u && parents[cell - params.width].root != root) { edges.z = 0.0; }
    if (cy + 1u < params.height && parents[cell + params.width].root != root) { edges.w = 0.0; }
    // Missing coarse support remains an anchor; it cannot become a cross-wall edge.
    parents[cell].edges = edges;
    parents[cell].diagonal = anchor + boundary.x + boundary.y + boundary.z + boundary.w;
}
