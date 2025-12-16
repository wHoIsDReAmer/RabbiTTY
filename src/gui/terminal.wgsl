struct Uniforms {
    cell_size : vec2<f32>,
    viewport  : vec2<f32>,
    offset    : vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms : Uniforms;

struct VertexIn {
    @location(0) quad_pos : vec2<f32>,
    @location(1) cell_pos : vec2<u32>,
    @location(2) color    : vec4<f32>,
};

struct VertexOut {
    @builtin(position) position : vec4<f32>,
    @location(0) color : vec4<f32>,
};

@vertex
fn vs_main(input : VertexIn) -> VertexOut {
    let cell = vec2<f32>(input.cell_pos);
    let pixel = (cell + input.quad_pos) * uniforms.cell_size + uniforms.offset;

    // Convert to NDC (origin top-left)
    let ndc = vec2<f32>(
        (pixel.x / uniforms.viewport.x) * 2.0 - 1.0,
        1.0 - (pixel.y / uniforms.viewport.y) * 2.0
    );

    var out : VertexOut;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input : VertexOut) -> @location(0) vec4<f32> {
    return input.color;
}
