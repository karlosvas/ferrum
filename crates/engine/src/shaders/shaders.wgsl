@group(1) @binding(0)
var<uniform> camera: CameraUniform;
@group(2) @binding(0)
var<uniform> light: Light;
@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;
@group(0) @binding(2)
var t_normal: texture_2d<f32>;
@group(0) @binding(3)
var s_normal: sampler;
@group(3) @binding(0)
var shadow_map: texture_depth_2d;
@group(3) @binding(1)
var shadow_sampler: sampler_comparison;

struct CameraUniform {
    view_pos: vec4<f32>,
    view: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
};

struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
    light_view_proj: mat4x4<f32>,
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
    @location(0) text_cords: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) world_normal: vec3<f32>,
    @location(3) world_position: vec3<f32>,
    @location(4) tangent_position: vec3<f32>,
    @location(5) tangent_light_position: vec3<f32>,
    @location(6) tangent_view_position: vec3<f32>,
    @location(7) light_space_pos: vec4<f32>,
}

@vertex
fn vs_main(
    model: VertexInput
) -> VertexOutput {
    let normal_matrix = mat3x3<f32>(model.normal_matrix_0, model.normal_matrix_1, model.normal_matrix_2);
    let world_normal = normalize(normal_matrix * model.normal);
    let world_tangent = normalize(normal_matrix * model.tangent);
    let world_bitangent = normalize(normal_matrix * model.bitangent);
    let tangent_matrix = transpose(mat3x3<f32>(
        world_tangent,
        world_bitangent,
        world_normal,
    ));

    let model_matrix = mat4x4<f32>(model.model_matrix_0, model.model_matrix_1, model.model_matrix_2, model.model_matrix_3);
    let world_position = model_matrix * vec4(model.position, 1.0);

    var out: VertexOutput;
    out.clip_position = camera.view_proj * world_position;
    out.text_cords = model.text_cords;
    out.world_normal = model.normal;
    out.color = model.color;
    out.world_position = world_position.xyz;
    out.tangent_position = tangent_matrix * world_position.xyz;
    out.tangent_view_position = tangent_matrix * camera.view_pos.xyz;
    out.tangent_light_position = tangent_matrix * light.position;
    out.light_space_pos = light.light_view_proj * world_position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let object_color: vec4<f32> = textureSample(t_diffuse, s_diffuse, in.text_cords);
    let object_normal: vec4<f32> = textureSample(t_normal, s_normal, in.text_cords);

    let tanget_normal = normalize(object_normal.xyz * 2.0 - 1.0);

    let ambient_strength = 0.1;
    let ambient_color = light.color * ambient_strength;

    let light_vec = in.tangent_light_position - in.tangent_position;
    let distance = length(light_vec);
    let light_dir = light_vec / distance;
    let view_dir = normalize(in.tangent_view_position - in.tangent_position);
    let half_dir  = normalize(view_dir + light_dir);

    // Distance attenuation: constant + linear + quadratic falloff (point light).
    // Tweak the coefficients to taste — bigger numbers = faster falloff.
    let attenuation = 1.0 / (1.0 + 0.09 * distance + 0.032 * distance * distance);

    let diffuse_strength = max(dot(tanget_normal, light_dir), 0.0);
    let diffuse_color = light.color * diffuse_strength * attenuation;

    let specular_strength = pow(max(dot(tanget_normal, half_dir), 0.0), 32.0);
    let specular_color = specular_strength * light.color * attenuation;

    // Shadow: project fragment into light-space NDC, then compare depth.
    let proj_coords = in.light_space_pos.xyz / in.light_space_pos.w;
    // NDC x/y [-1,1] → UV [0,1]; flip Y because texture V goes down.
    let shadow_uv = proj_coords.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let current_depth = proj_coords.z - 0.002; // small bias to avoid acne

    var shadow_factor = 1.0;
    if (shadow_uv.x >= 0.0 && shadow_uv.x <= 1.0 &&
        shadow_uv.y >= 0.0 && shadow_uv.y <= 1.0 &&
        current_depth >= 0.0 && current_depth <= 1.0) {
        // Returns 1.0 if fragment is lit (depth <= shadow map), 0.0 if in shadow.
        shadow_factor = textureSampleCompare(shadow_map, shadow_sampler, shadow_uv, current_depth);
    }

    let result = (ambient_color + shadow_factor * (diffuse_color + specular_color)) * object_color.rgb * in.color;
    return vec4<f32>(result, object_color.a);
}
