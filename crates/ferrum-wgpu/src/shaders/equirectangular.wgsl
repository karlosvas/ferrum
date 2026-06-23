// Equirectangular HDR -> cubemap conversion (compute shader).
// Based on Learn WGPU's HDR tutorial:
//   https://sotrh.github.io/learn-wgpu/intermediate/tutorial13-hdr/
// Direction -> equirect UV mapping (the 0.1591/0.3183 = 1/2pi, 1/pi trick) from LearnOpenGL IBL:
//   https://learnopengl.com/PBR/IBL/Diffuse-irradiance

struct Face {
    forward: vec3<f32>,
    up: vec3<f32>,
    right: vec3<f32>,
}

@group(0)
@binding(0)
var src: texture_2d<f32>;

@group(0)
@binding(1)
var dst: texture_storage_2d_array<rgba16float, write>;

// Manual bilinear filter (textureLoad has no sampler in compute).
fn sample_bilinear(coord: vec2<f32>, max_coord: vec2<i32>) -> vec4<f32> {
    let i0 = vec2<i32>(floor(coord));
    let f = fract(coord);
    let s00 = textureLoad(src, clamp(i0, vec2<i32>(0), max_coord), 0);
    let s10 = textureLoad(src, clamp(i0 + vec2<i32>(1, 0), vec2<i32>(0), max_coord), 0);
    let s01 = textureLoad(src, clamp(i0 + vec2<i32>(0, 1), vec2<i32>(0), max_coord), 0);
    let s11 = textureLoad(src, clamp(i0 + vec2<i32>(1, 1), vec2<i32>(0), max_coord), 0);
    return mix(mix(s00, s10, f.x), mix(s01, s11, f.x), f.y);
}

@compute
@workgroup_size(16, 16, 1)
fn compute_equirect_to_cubemap(
    @builtin(global_invocation_id)
    gid: vec3<u32>,
) {
    // Skip threads outside the texture when its size is not a multiple of 16.
    if gid.x >= u32(textureDimensions(dst).x) {
        return;
    }

    var FACES: array<Face, 6> = array(
        // FACES +X
        Face(
            vec3(1.0, 0.0, 0.0),  // forward
            vec3(0.0, 1.0, 0.0),  // up
            vec3(0.0, 0.0, -1.0), // right
        ),
        // FACES -X
        Face (
            vec3(-1.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
            vec3(0.0, 0.0, 1.0),
        ),
        // FACES +Y
        Face (
            vec3(0.0, -1.0, 0.0),
            vec3(0.0, 0.0, 1.0),
            vec3(1.0, 0.0, 0.0),
        ),
        // FACES -Y
        Face (
            vec3(0.0, 1.0, 0.0),
            vec3(0.0, 0.0, -1.0),
            vec3(1.0, 0.0, 0.0),
        ),
        // FACES +Z
        Face (
            vec3(0.0, 0.0, 1.0),
            vec3(0.0, 1.0, 0.0),
            vec3(1.0, 0.0, 0.0),
        ),
        // FACES -Z
        Face (
            vec3(0.0, 0.0, -1.0),
            vec3(0.0, 1.0, 0.0),
            vec3(-1.0, 0.0, 0.0),
        ),
    );

    // Get texture coords relative to cubemap face
    let dst_dimensions = vec2<f32>(textureDimensions(dst));
    let cube_uv = vec2<f32>(gid.xy) / dst_dimensions * 2.0 - 1.0;

    // Get spherical coordinate from cube_uv
    let face = FACES[gid.z];
    let spherical = normalize(face.forward + face.right * cube_uv.x + face.up * cube_uv.y);

    // Get coordinate on the equirectangular texture
    let inv_atan = vec2(0.1591, 0.3183);
    let eq_uv = vec2(atan2(spherical.z, spherical.x), asin(spherical.y)) * inv_atan + 0.5;

    let src_dim = vec2<f32>(textureDimensions(src));
    let c = eq_uv * src_dim - 0.5;
    let max_coord = vec2<i32>(src_dim) - vec2<i32>(1);

    // Box filter: 4 bilinear samples shifted 1 pixel each, averaged.
    // Covers an effective 3x3 window to reduce aliasing from the heavy downsample.
    var acc = vec4<f32>(0.0);
    for (var dy: i32 = 0; dy < 2; dy++) {
        for (var dx: i32 = 0; dx < 2; dx++) {
            acc += sample_bilinear(c + vec2<f32>(f32(dx), f32(dy)), max_coord);
        }
    }
    let sample = acc * 0.25;

    textureStore(dst, gid.xy, gid.z, sample);
}
