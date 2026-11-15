// One rounded rectangle per instance: filled or a ring, a vertical two-stop gradient, times a texture.
struct Globals { size: vec2<f32>, pad: vec2<f32> }
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var samp: sampler;
@group(1) @binding(0) var tex: texture_2d<f32>;

struct Inst {
    @location(0) rect: vec4<f32>,   // x, y, w, h in pixels
    @location(1) c0: vec4<f32>,     // top colour, linear, straight alpha
    @location(2) c1: vec4<f32>,     // bottom colour
    @location(3) params: vec4<f32>, // corner radius, ring thickness (0 = filled), unused, unused
}

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) half: vec2<f32>,
    @location(2) @interpolate(flat) c0: vec4<f32>,
    @location(3) @interpolate(flat) c1: vec4<f32>,
    @location(4) @interpolate(flat) params: vec4<f32>,
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, i: Inst) -> VOut {
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let half = i.rect.zw * 0.5;
    // One pixel wider than the shape, for the antialiased edge.
    let local = (corner * 2.0 - 1.0) * (half + 1.0);
    let p = i.rect.xy + half + local;
    var o: VOut;
    o.pos = vec4<f32>(p.x / g.size.x * 2.0 - 1.0, 1.0 - p.y / g.size.y * 2.0, 0.0, 1.0);
    o.local = local;
    o.half = half;
    o.c0 = i.c0;
    o.c1 = i.c1;
    o.params = i.params;
    return o;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let r = min(in.params.x, min(in.half.x, in.half.y));
    let q = abs(in.local) - in.half + r;
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
    var a = clamp(0.5 - d, 0.0, 1.0);
    if (in.params.y > 0.0) {
        a *= clamp(0.5 + d + in.params.y, 0.0, 1.0);
    }
    let uv = (in.local + in.half) / (2.0 * in.half);
    let c = mix(in.c0, in.c1, clamp(uv.y, 0.0, 1.0)) * textureSample(tex, samp, uv);
    return vec4<f32>(c.rgb * c.a * a, c.a * a);
}
