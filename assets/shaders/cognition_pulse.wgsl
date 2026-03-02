// cognition_pulse.wgsl
// Transmuted by Bzzrts (The Prism) under the Edict of Magos Pixelis

struct Uniforms {
    color: vec4<f32>,
    time: f32,
    intensity: f32, // Maps to bloom and alpha scaling
    frequency: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // 1. Center the vertical coordinate
    let y = input.uv.y - 0.5;
    
    // 2. Drive the ripple/displacement
    // Wave speed and frequency based on the machine state
    let wave = sin(input.uv.x * 10.0 + uniforms.time * uniforms.frequency * 6.28318);
    
    // 3. Gaussian Falloff Logic
    // The "Height of Sanctity" is 8px. In UV space (0.0 to 1.0), 
    // we calculate the distance from the center line, displaced by the wave.
    let dist = abs(y - (wave * 0.05 * uniforms.intensity));
    
    // 4. Bloom and Thickness
    // falloff = exp(-dist^2 / sigma^2)
    // Sigma controls the thickness. We scale it with intensity for the "Great Bloom".
    let sigma = 0.1 + (0.1 * uniforms.intensity);
    let falloff = exp(-(dist * dist) / (sigma * sigma));
    
    // 5. Final Color Synthesis
    // Apply the sacred palette color and scale alpha by intensity and falloff.
    var final_color = uniforms.color;
    final_color.a = falloff * uniforms.intensity;
    
    return final_color;
}
