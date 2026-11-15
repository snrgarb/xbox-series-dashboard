// One pass of a Gaussian blur along one axis: a layer drawn to a texture of its own is blurred across into a second
// texture, then down as it is drawn onto the target, where it is faded as one picture. The texture holds premultiplied
// colour, so its clear surround blurs in as nothing and the result is blended as the quads are.
struct Params {
    rect: vec4<f32>,  // where on the target the texture is drawn: x, y, w, h in pixels
    size: vec2<f32>,  // the target's size in pixels
    step: vec2<f32>,  // one texel along the pass's axis, in the texture's 0 to 1
    sigma: f32,       // the Gaussian's deviation in texels
    taps: f32,        // how many texels either side of the middle are summed
    opacity: f32,     // what the whole picture is multiplied by
    pad: f32,
}
@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var tex: texture_2d<f32>;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VOut {
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let at = p.rect.xy + corner * p.rect.zw;
    var o: VOut;
    o.pos = vec4<f32>(at.x / p.size.x * 2.0 - 1.0, 1.0 - at.y / p.size.y * 2.0, 0.0, 1.0);
    o.uv = corner;
    return o;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    var sum = vec4<f32>(0.0);
    var weights = 0.0;
    let taps = i32(p.taps);
    for (var i = -taps; i <= taps; i++) {
        let x = f32(i) / p.sigma;
        let weight = exp(-0.5 * x * x);
        sum += weight * textureSampleLevel(tex, samp, in.uv + p.step * f32(i), 0.0);
        weights += weight;
    }
    return sum / weights * p.opacity;
}
