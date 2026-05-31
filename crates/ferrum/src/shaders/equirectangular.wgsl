const PI: f32 = 3.1415926535897932384626433832795;

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

@compute
@workgroup_size(16, 16, 1)
fn compute_equirect_to_cubemap(
    @builtin(global_invocation_id)
    gid: vec3<u32>,
) {
    // If texture size is not divisible by 32, we
    // need to make sure we don't try to write to
    // pixels that don't exist.
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
    let c_before = eq_uv * src_dim;
    let c = c_before - vec2<f32>(0.5);

    let i0 = vec2<i32>(floor(c));
    let f  = fract(c);

    // i0, i0+1..
    let max_coord = vec2<i32>(src_dim) - vec2<i32>(1);

    // Filtro de caja: 4 muestras bilineares desplazadas 1 píxel cada una,
    // promediadas. Cubre una ventana efectiva de 3x3 píxeles del source y
    // mata el aliasing residual del downsample fuerte.
    var acc = vec4<f32>(0.0);
    for (var dy: i32 = 0; dy < 2; dy++) {
        for (var dx: i32 = 0; dx < 2; dx++) {
            let c_shift = c + vec2<f32>(f32(dx), f32(dy));
            let i0_s = vec2<i32>(floor(c_shift));
            let f_s  = fract(c_shift);

            let s00 = textureLoad(src, clamp(i0_s + vec2<i32>(0, 0), vec2<i32>(0), max_coord), 0);
            let s10 = textureLoad(src, clamp(i0_s + vec2<i32>(1, 0), vec2<i32>(0), max_coord), 0);
            let s01 = textureLoad(src, clamp(i0_s + vec2<i32>(0, 1), vec2<i32>(0), max_coord), 0);
            let s11 = textureLoad(src, clamp(i0_s + vec2<i32>(1, 1), vec2<i32>(0), max_coord), 0);

            let sx0 = mix(s00, s10, f_s.x);
            let sx1 = mix(s01, s11, f_s.x);
            acc += mix(sx0, sx1, f_s.y);
        }
    }
    let sample = acc * 0.25;

    textureStore(dst, gid.xy, gid.z, sample);
}
