struct Camera {
    view_pos: vec4<f32>,
    view: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
}
@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1)
@binding(0)
var env_map: texture_cube<f32>;
@group(1)
@binding(1)
var env_sampler: sampler;

struct VertexOutput {
    @builtin(position) frag_position: vec4<f32>,
    @location(0) clip_position: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) id: u32,
) -> VertexOutput {
    let uv = vec2<f32>(vec2<u32>(
        id & 1u,
        (id >> 1u) & 1u,
    ));
    var out: VertexOutput;
    // out.clip_position = vec4(uv * vec2(4.0, -4.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    out.clip_position = vec4(uv * 4.0 - 1.0, 1.0, 1.0);
    out.frag_position = vec4(uv * 4.0 - 1.0, 1.0, 1.0);
    return out;
}

// Sky exposure: 1.0 = day (original HDR color).
// Lower it to darken: ~0.3 dusk, ~0.05 night, 0.0 fully black.
const SKY_EXPOSURE: f32 = 0.01;
// Night tint (dark blue). Only blends in when SKY_EXPOSURE is low.
const NIGHT_TINT: vec3<f32> = vec3<f32>(0.05, 0.08, 0.20);

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let view_pos_homogeneous = camera.inv_proj * in.clip_position;
    let view_ray_direction = view_pos_homogeneous.xyz / view_pos_homogeneous.w;
    var ray_direction = normalize((camera.inv_view * vec4(view_ray_direction, 0.0)).xyz);

    let sample = textureSample(env_map, env_sampler, ray_direction);

    // Night/day blend: lower exposure gives more weight to the dark blue tint.
    let night_blend = clamp(1.0 - SKY_EXPOSURE, 0.0, 1.0);
    let rgb = sample.rgb * SKY_EXPOSURE + NIGHT_TINT * night_blend;
    return vec4<f32>(rgb, sample.a);
}
