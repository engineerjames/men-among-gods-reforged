#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> color_keys: array<vec3<f32>, 9>;
@group(2) @binding(1) var texture: texture_2d<f32>;
@group(2) @binding(2) var texture_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(texture, texture_sampler, mesh.uv);
    
    // Convert sampled color from 0.0-1.0 to 0-255 range for comparison
    let color_255 = round(color.rgb * 255.0);
    
    // Check if pixel matches any of the color keys with small tolerance
    for (var i = 0u; i < 9u; i++) {
        let diff = abs(color_255 - color_keys[i]);
        // Allow 1-2 units tolerance for anti-aliased edges
        if (diff.r <= 10.0 && diff.g <= 10.0 && diff.b <= 10.0) {
            discard;
        }
    }
    
    // Also discard magenta-like colors: red ≈ blue and green is low
    let rb_diff = abs(color_255.r - color_255.b);
    let avg_rb = (color_255.r + color_255.b) / 2.0;
    // If red and blue are similar and much higher than green, discard
    if (rb_diff <= 15.0 && color_255.g < 30.0 && avg_rb > 200.0) {
        discard;
    }
    
    return color;
}
