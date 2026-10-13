// Shared 32-byte resident node ABI. Weights are west, east, north and south.
struct Node {
    edges: vec4<f32>,
    diagonal: f32,
    root: u32,
    count: u32,
    padding: u32,
}
