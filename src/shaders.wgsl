// struct VertexInput {
//     @location(0) position: vec3<f32>,
//     @location(1) color: vec3<f32>,
// }

// struct VertexOutput {
//     @builtin(position) clip_position: vec4<f32>,
//     @location(0) color: vec3<f32>,
// }

// @vertex
// fn vs_main(
//     model: VertexInput
// ) -> VertexOutput {
//     var output: VertexOutput;
//     output.color = model.color;
//     output.clip_position = vec4<f32>(model.position, 1.0);
//     return output;
// }

// @fragment
// fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
//     return vec4<f32>(in.color, 1);
// }

struct CameraUniform{
    view_proj: mat4x4<f32>
};

@group(1) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) text_cords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) text_cords: vec2<f32>,
}

@vertex
fn vs_main(
    model: VertexInput
) -> VertexOutput {
    var output: VertexOutput;
    output.text_cords = model.text_cords;
    output.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
    return output;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;

@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.text_cords);
}

