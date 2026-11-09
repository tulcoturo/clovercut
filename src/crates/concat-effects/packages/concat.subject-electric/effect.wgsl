struct Params { amount: f32, width: f32, hue: f32, speed: f32 }

fn person_tint(hue: f32) -> vec3<f32> {
    let p = abs(fract(vec3<f32>(hue / 360.0) + vec3<f32>(0.0, 0.6666667, 0.3333333)) * 6.0 - 3.0);
    return mix(vec3<f32>(0.08), clamp(p - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(0.92));
}

// Bounded, derivative-free sampling also works on Windows' software adapter.
fn person_surround(uv: vec2<f32>, radius: f32) -> f32 {
    let delta = vec2<f32>(radius * frame.size.y) / frame.size;
    var keep = subject_at(uv);
    for (var i = 0; i < 8; i++) {
        let angle = f32(i) * 0.7853981634;
        keep = max(keep, subject_at(uv + vec2<f32>(cos(angle), sin(angle)) * delta));
    }
    return keep;
}

fn person_lit(base: vec4<f32>, emission: vec3<f32>, cover: f32) -> vec4<f32> {
    let alpha = max(base.a, clamp(cover, 0.0, 1.0));
    if (alpha <= 0.000001) { return base; }
    // Straight alpha at the boundary, extended scene-linear light throughout.
    return vec4<f32>((base.rgb * base.a + emission) / alpha, alpha);
}

fn person_hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(123.34, 456.21));
    let v = q + dot(q, q + vec2<f32>(45.32));
    return fract(v.x * v.y);
}
fn person_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (vec2<f32>(3.0) - 2.0 * f);
    return mix(mix(person_hash(i), person_hash(i + vec2<f32>(1.0,0.0)), u.x),
        mix(person_hash(i + vec2<f32>(0.0,1.0)), person_hash(i + vec2<f32>(1.0,1.0)), u.x), u.y);
}
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let base = sample(uv);
    if (params.amount <= 0.0) { return base; }
    let centre = subject_at(uv);
    let r = params.width * 0.01;
    let t = frame.clip_time * params.speed;
    let p = uv * frame.size / frame.size.y;
    let near = max(0.0, person_surround(uv, r * 0.22) - centre);
    let band = max(0.0, person_surround(uv, r) - centre);
    let halo = max(0.0, person_surround(uv, r * 1.8) - centre);
    let flow = person_noise(p * 23.0 + vec2<f32>(t * 2.2, -t * 4.0));
    let zigzag = person_noise(p * 74.0 + vec2<f32>(flow * 3.0, t * 5.0));
    let bolt = pow(max(0.0, 1.0 - abs(zigzag - 0.5) * 2.0), 14.0) * band;
    let sparks = step(0.94, person_noise(p * 180.0 + vec2<f32>(floor(t * 12.0)))) * halo;
    let amount = params.amount * 0.01;
    let colour = person_tint(params.hue);
    let bright = near * (0.5 + flow) + bolt * 2.5 + sparks * 1.5;
    let emission = amount * (colour * (halo * 0.12 + bright) + vec3<f32>(0.75) * bolt);
    return person_lit(base, emission, amount * (bright + halo * 0.15));
}
