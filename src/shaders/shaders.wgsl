// Main lit shader: tangent-space normal mapping + Blinn-Phong + shadow mapping + wind sway.
// Normal mapping / TBN matrix based on Learn WGPU and LearnOpenGL:
//   https://sotrh.github.io/learn-wgpu/intermediate/tutorial11-normals/
//   https://learnopengl.com/Advanced-Lighting/Normal-Mapping

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
@group(4) @binding(0)
var<uniform> wind: Wind;

struct Wind {
    direction: vec2<f32>, 
    intensity: f32,       
    time: f32,
};

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
    @location(13) wind_weight: f32,
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

    // Wind sway: height-weighted sine displacement in the vertex shader,
    // same idea as GPU Gems 3 ch. 16 (vegetation animation in Crysis):
    // https://developer.nvidia.com/gpugems/gpugems3/part-iii-rendering/chapter-16-vegetation-procedural-animation-and-shading-crysis
    let sway_amplitude = 0.5;
    let reference_height = 6.0;
    var local_pos = model.position;
    let h = clamp(max(local_pos.y, 0.0) / reference_height, 0.0, 1.0);
    let height_weight = h * h;
    let phase = local_pos.x * 0.7 + local_pos.z * 0.7;
    let sway = 0.65
             + sin(wind.time * 2.3 + phase) * 0.25       
             + sin(wind.time * 5.7 + phase * 1.7) * 0.10;  
    let displacement = wind.direction
        * (sway * wind.intensity * height_weight * model.wind_weight * sway_amplitude);
    local_pos.x += displacement.x;
    local_pos.z += displacement.y;

    let world_position = model_matrix * vec4(local_pos, 1.0);

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

    let ambient_strength = 0.05;
    let ambient_color = light.color * ambient_strength;

    let light_vec = in.tangent_light_position - in.tangent_position;
    let distance = length(light_vec);
    let light_dir = light_vec / distance;
    let view_dir = normalize(in.tangent_view_position - in.tangent_position);
    let half_dir  = normalize(view_dir + light_dir);

    // Point-light attenuation, coefficients from the Ogre3D table (range ~50):
    // https://learnopengl.com/Lighting/Light-casters
    let attenuation = 1.0 / (1.0 + 0.09 * distance + 0.032 * distance * distance);

    let diffuse_strength = max(dot(tanget_normal, light_dir), 0.0);
    let diffuse_color = light.color * diffuse_strength * attenuation;

    // Blinn-Phong specular (half vector instead of reflection):
    // https://learnopengl.com/Advanced-Lighting/Advanced-Lighting
    let specular_strength = pow(max(dot(tanget_normal, half_dir), 0.0), 32.0);
    let specular_color = specular_strength * light.color * attenuation;

    // Shadow mapping: project into light space, compare depth, slope-scaled bias,
    // 3x3 PCF for soft edges. All techniques explained in:
    // https://learnopengl.com/Advanced-Lighting/Shadows/Shadow-Mapping
    let proj_coords = in.light_space_pos.xyz / in.light_space_pos.w;
    // NDC x/y [-1,1] -> UV [0,1]; flip Y because texture V goes down.
    let shadow_uv = proj_coords.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let cos_theta = clamp(dot(normalize(in.world_normal), normalize(light.position - in.world_position)), 0.0, 1.0);
    let shadow_bias = mix(0.004, 0.0002, cos_theta);
    let current_depth = proj_coords.z - shadow_bias;

    // textureSampleCompare needs uniform control flow, so always sample (the
    // constant-bound loop is uniform; ClampToEdge makes out-of-range UVs safe)
    // and only afterwards decide if the fragment is inside the light frustum.
    let texel = 1.0 / 2048.0;
    var shadow_sum = 0.0;
    for (var y: i32 = -1; y <= 1; y++) {
        for (var x: i32 = -1; x <= 1; x++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel;
            shadow_sum += textureSampleCompare(shadow_map, shadow_sampler, shadow_uv + offset, current_depth);
        }
    }

    // Fragments outside the light frustum (or beyond its depth range) are fully lit.
    let in_bounds = shadow_uv.x >= 0.0 && shadow_uv.x <= 1.0 &&
                    shadow_uv.y >= 0.0 && shadow_uv.y <= 1.0 &&
                    current_depth >= 0.0 && current_depth <= 1.0;
    let shadow_factor = select(1.0, shadow_sum / 9.0, in_bounds);

    let result = (ambient_color + shadow_factor * (diffuse_color + specular_color)) * object_color.rgb * in.color;
    return vec4<f32>(result, object_color.a);
}
