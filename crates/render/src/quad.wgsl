// One rounded rectangle per instance: filled, a ring or an arc of one, a two-stop gradient, times a texture, cut to a
// clip rectangle, and turned about its middle.
// Colours are sRGB values with straight alpha and are blended as they are, in gamma space, as WPF blends: the theme's
// colours and alphas carry over unchanged.
// The target's size in pixels, and where its top left corner is in the frame's own pixels: the origin, but for a layer
// drawn to a texture of its own to be blurred, which holds a part of the frame.
struct Globals { size: vec2<f32>, origin: vec2<f32> }
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var samp: sampler;
@group(1) @binding(0) var tex: texture_2d<f32>;

struct Inst {
    @location(0) rect: vec4<f32>,   // x, y, w, h in pixels
    @location(1) c0: vec4<f32>,     // the gradient's first colour
    @location(2) c1: vec4<f32>,     // its second
    @location(3) params: vec4<f32>, // corner radius, ring thickness (0 = filled), gradient kind (4 = an arc), feather
    @location(4) uv: vec4<f32>,     // the texture's u0, v0, u1, v1 across the rectangle; an arc's middle and half length
    @location(5) clip: vec4<f32>,   // x0, y0, x1, y1 in pixels
    @location(6) turn: vec4<f32>,   // the cosine and sine of the turn about the rectangle's middle, clockwise
}

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) half: vec2<f32>,
    @location(2) @interpolate(flat) c0: vec4<f32>,
    @location(3) @interpolate(flat) c1: vec4<f32>,
    @location(4) @interpolate(flat) params: vec4<f32>,
    @location(5) @interpolate(flat) uv: vec4<f32>,
    @location(6) @interpolate(flat) clip: vec4<f32>,
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, i: Inst) -> VOut {
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let half = i.rect.zw * 0.5;
    // Wider than the shape by its soft edge.
    let local = (corner * 2.0 - 1.0) * (half + max(i.params.w, 1.0));
    let turned = vec2<f32>(local.x * i.turn.x - local.y * i.turn.y, local.x * i.turn.y + local.y * i.turn.x);
    let p = i.rect.xy + half + turned - g.origin;
    var o: VOut;
    o.pos = vec4<f32>(p.x / g.size.x * 2.0 - 1.0, 1.0 - p.y / g.size.y * 2.0, 0.0, 1.0);
    o.local = local;
    o.half = half;
    o.c0 = i.c0;
    o.c1 = i.c1;
    o.params = i.params;
    o.uv = i.uv;
    o.clip = i.clip;
    return o;
}

// The quad's colour where its texture reads `texel`.
fn shade(in: VOut, texel: vec4<f32>) -> vec4<f32> {
    let r = min(in.params.x, min(in.half.x, in.half.y));
    let q = abs(in.local) - in.half + r;
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
    let feather = max(in.params.w, 1.0);
    var a = clamp(0.5 - d / feather, 0.0, 1.0);
    if (in.params.y > 0.0) {
        a *= clamp(0.5 + (d + in.params.y) / feather, 0.0, 1.0);
    }
    if (feather > 1.0) {
        a = a * a * (3.0 - 2.0 * a);
    }
    if (in.params.z == 4.0) {
        // An arc with round ends: the ring's centre line from uv.x - uv.y to uv.x + uv.y radians clockwise from the
        // top. In a frame whose y axis runs through the arc's middle it is symmetric, so one half is worked out.
        let stroke = in.params.y * 0.5;
        let line = min(in.half.x, in.half.y) - stroke;
        let middle = vec2<f32>(sin(in.uv.x), -cos(in.uv.x));
        let p = vec2<f32>(abs(in.local.x * middle.y - in.local.y * middle.x), dot(in.local, middle));
        let end = vec2<f32>(sin(in.uv.y), cos(in.uv.y));
        var along = abs(length(p) - line);
        if (end.y * p.x > end.x * p.y) {
            along = length(p - end * line);
        }
        a = clamp(0.5 - (along - stroke) / feather, 0.0, 1.0);
    }
    let at = in.pos.xy + g.origin;
    let cut = min(at - in.clip.xy, in.clip.zw - at);
    a *= clamp(min(cut.x, cut.y) + 0.5, 0.0, 1.0);

    let unit = clamp((in.local + in.half) / (2.0 * in.half), vec2<f32>(0.0), vec2<f32>(1.0));
    var t = unit.y;
    if (in.params.z == 1.0) {
        t = unit.x;
    } else if (in.params.z == 2.0) {
        t = (unit.x + unit.y) * 0.5;
    } else if (in.params.z == 3.0) {
        t = clamp(length(unit * 2.0 - 1.0), 0.0, 1.0);
    }
    let c = mix(in.c0, in.c1, t) * texel;
    return vec4<f32>(c.rgb * c.a * a, c.a * a);
}

fn texel_at(in: VOut) -> vec4<f32> {
    let unit = clamp((in.local + in.half) / (2.0 * in.half), vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(tex, samp, mix(in.uv.xy, in.uv.zw, unit));
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    return shade(in, texel_at(in));
}

// For a video texture, whose fourth byte is whatever the decoder left there: the picture is opaque.
@fragment
fn fs_opaque(in: VOut) -> @location(0) vec4<f32> {
    return shade(in, vec4<f32>(texel_at(in).rgb, 1.0));
}
