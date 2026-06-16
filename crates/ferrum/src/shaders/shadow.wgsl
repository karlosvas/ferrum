// Depth-only pass that renders the scene from the light's point of view
// to build the shadow map (first pass of classic shadow mapping):
//   https://learnopengl.com/Advanced-Lighting/Shadows/Shadow-Mapping

@group(0) @binding(0)
var<uniform> light: Light;

struct Light {
    position: vec3<f32>,
    color: vec3<f32>,   // unused here but needed to push light_view_proj to offset 32
    _pad: u32,
    light_view_proj: mat4x4<f32>
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) text_cords: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) color: vec3<f32>,
    @location(4) tangent: vec3<f32>,
    @location(5) bitangent: vec3<f32>,
    @location(6)  model_matrix_0: vec4<f32>,
    @location(7)  model_matrix_1: vec4<f32>,
    @location(8)  model_matrix_2: vec4<f32>,
    @location(9)  model_matrix_3: vec4<f32>,
    @location(10) normal_matrix_0: vec3<f32>,
    @location(11) normal_matrix_1: vec3<f32>,
    @location(12) normal_matrix_2: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@vertex
fn vs_main(
    model: VertexInput
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(model.model_matrix_0, model.model_matrix_1, model.model_matrix_2, model.model_matrix_3);
    let world_position = model_matrix * vec4(model.position, 1.0);

    var out: VertexOutput;
    out.clip_position = light.light_view_proj * world_position;
    return out;
}
