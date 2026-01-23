#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct Params {
    screen_size: vec2<f32>,
    source_count: u32,
    magic_enabled: u32,
    gamma: f32,
    _pad0: vec3<f32>,
};

struct MagicSource {
    pos: vec2<f32>,
    strength: u32,
    mask: u32,
};

@group(2) @binding(0) var scene_tex: texture_2d<f32>;
@group(2) @binding(1) var scene_sampler: sampler;
@group(2) @binding(2) var<uniform> params: Params;
@group(2) @binding(3) var<storage, read> sources: array<MagicSource>;

fn apply_magic_one(
    r_in: i32,
    g_in: i32,
    b_in: i32,
    src: MagicSource,
    px: vec2<f32>,
) -> vec3<i32> {
    // Compute pixel offset within the 64x64 region.
    let dx = i32(floor(px.x)) - i32(round(src.pos.x));
    let dy = i32(floor(px.y)) - i32(round(src.pos.y));

    if (dx < 0 || dx >= 64 || dy < 0 || dy >= 64) {
        return vec3<i32>(r_in, g_in, b_in);
    }

    var e = 32;
    if (dx < 32) { e = e - (32 - dx); }
    if (dx > 31) { e = e - (dx - 31); }
    if (dy < 16) { e = e - (16 - dy); }
    if (dy > 55) { e = e - ((dy - 55) * 2); }
    if (e < 0) { e = 0; }

    let str = max(1, i32(src.strength));
    e = e / str;
    if (e <= 0) {
        return vec3<i32>(r_in, g_in, b_in);
    }

    var e2 = 0;
    if ((src.mask & 1u) != 0u) { e2 = e2 + e; }
    if ((src.mask & 2u) != 0u) { e2 = e2 + e; }
    if ((src.mask & 4u) != 0u) { e2 = e2 + e; }

    var r = r_in - (e2 / 2);
    var g = g_in - e2;
    var b = b_in - (e2 / 2);

    if (r < 0) { r = 0; }
    if (g < 0) { g = 0; }
    if (b < 0) { b = 0; }

    if ((src.mask & 1u) != 0u) {
        r = r + e;
        if (r > 31) { r = 31; }
    }

    if ((src.mask & 2u) != 0u) {
        g = g + (e * 2);
        if (g > 63) { g = 63; }
    }

    if ((src.mask & 4u) != 0u) {
        b = b + e;
        if (b > 31) { b = 31; }
    }

    return vec3<i32>(r, g, b);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let px = uv * params.screen_size;

    var out = textureSample(scene_tex, scene_sampler, uv);

    // Apply magic only when enabled and in the classic region.
    if (params.magic_enabled != 0u && px.y >= 200.0 && params.source_count > 0u) {
        // Convert to RGB565-like integer space.
        var r = i32(round(clamp(out.r, 0.0, 1.0) * 31.0));
        var g = i32(round(clamp(out.g, 0.0, 1.0) * 63.0));
        var b = i32(round(clamp(out.b, 0.0, 1.0) * 31.0));

        let count = params.source_count;
        for (var i = 0u; i < count; i = i + 1u) {
            let rgb = apply_magic_one(r, g, b, sources[i], px);
            r = rgb.x;
            g = rgb.y;
            b = rgb.z;
        }

        out = vec4<f32>(f32(r) / 31.0, f32(g) / 63.0, f32(b) / 31.0, out.a);
    }

    // Gamma correction (applied even when magic disabled).
    let g = max(0.001, params.gamma);
    let inv_gamma = 1.0 / g;
    let rgb = pow(max(out.rgb, vec3<f32>(0.0)), vec3<f32>(inv_gamma));
    return vec4<f32>(rgb, out.a);
}
