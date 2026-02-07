#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Mirrors the effect-bit model used by world_render.rs:
// - low bits: numeric light level (0..1023)
// - bit  16: highlight
// - bit  32: green (selection)
// - bit  64: invis
// - bit 128: grey (stoned)
// - bit 256: infrared
// - bit 512: underwater

struct Params {
    effect: u32,
    _pad0: vec3<f32>,
};

@group(2) @binding(0) var sprite_tex: texture_2d<f32>;
@group(2) @binding(1) var sprite_sampler: sampler;
@group(2) @binding(2) var<uniform> params: Params;

// dd.c lighting approximation used by the existing CPU tint path.
const DD_LEFFECT: f32 = 120.0;

fn strip_flag(base_in: u32, bit: u32) -> u32 {
    if ((base_in & bit) != 0u) {
        return base_in - bit;
    }
    return base_in;
}

fn effect_base(effect: u32) -> u32 {
    var base = effect;
    base = strip_flag(base, 16u);
    base = strip_flag(base, 32u);
    base = strip_flag(base, 64u);
    base = strip_flag(base, 128u);
    base = strip_flag(base, 256u);
    base = strip_flag(base, 512u);
    return base;
}

fn dd_shade(effect: u32) -> f32 {
    let base = effect_base(effect);
    let e = f32(min(base, 1023u));
    if (e <= 0.0) {
        return 1.0;
    }
    return DD_LEFFECT / (e * e + DD_LEFFECT);
}

fn apply_effect(color: vec4<f32>, effect: u32) -> vec4<f32> {
    let green = (effect & 32u) != 0u;
    let invis = (effect & 64u) != 0u;
    let grey = (effect & 128u) != 0u;
    let infra = (effect & 256u) != 0u;
    let water = (effect & 512u) != 0u;

    // Base lighting (applied per-pixel).
    var rgb = color.rgb * dd_shade(effect);

    // Per-pixel greyscale conversion (stoned), with a slight green bias like the CPU path.
    if (grey) {
        let lum = dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
        rgb = lum * vec3<f32>(0.85, 0.95, 0.85);
    }

    // Infrared: keep red channel only.
    if (infra) {
        rgb = vec3<f32>(rgb.r, 0.0, 0.0);
    }

    // Underwater: bias toward blue.
    if (water) {
        rgb = vec3<f32>(rgb.r * 0.6, rgb.g * 0.6, rgb.b * 1.2);
    }

    // "Green" effect: reduce red/blue to bias green.
    if (green) {
        rgb = vec3<f32>(rgb.r * 0.6, rgb.g, rgb.b * 0.6);
    }

    // Invis: black-out (keep alpha so sprites can still shape-mask).
    if (invis) {
        rgb = vec3<f32>(0.0, 0.0, 0.0);
    }

    // Keep values reasonable (Bevy clamps later, but avoid huge HDR spikes).
    rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.35));
    return vec4<f32>(rgb, color.a);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(sprite_tex, sprite_sampler, mesh.uv);
    return apply_effect(color, params.effect);
}
