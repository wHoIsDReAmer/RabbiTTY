struct Uniforms {
    viewport : vec2<f32>,
    unused   : vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms : Uniforms;
@group(0) @binding(1)
var image_sampler : sampler;
@group(0) @binding(2)
var image_texture : texture_2d<f32>;

struct VertexIn {
    @location(0) quad_pos : vec2<f32>,
    @location(1) pos      : vec2<f32>,
    @location(2) size     : vec2<f32>,
    @location(3) uv_min   : vec2<f32>,
    @location(4) uv_max   : vec2<f32>,
};

struct VertexOut {
    @builtin(position) position : vec4<f32>,
    @location(0) uv : vec2<f32>,
};

@vertex
fn vs_main(input : VertexIn) -> VertexOut {
    let pixel = input.pos + input.quad_pos * input.size;
    let ndc = vec2<f32>(
        (pixel.x / uniforms.viewport.x) * 2.0 - 1.0,
        1.0 - (pixel.y / uniforms.viewport.y) * 2.0
    );

    var out : VertexOut;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = input.uv_min + (input.uv_max - input.uv_min) * input.quad_pos;
    return out;
}

@fragment
fn fs_main(input : VertexOut) -> @location(0) vec4<f32> {
    return textureSample(image_texture, image_sampler, input.uv);
}
