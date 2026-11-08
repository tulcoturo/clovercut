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

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let base = sample(uv);
    if (params.amount <= 0.0) { return base; }
    let centre = subject_at(uv);
    let r = params.width * 0.01;
    let core = max(0.0, person_surround(uv, r * 0.3) - centre);
    let glow = max(0.0, 0.65 * person_surround(uv, r) + 0.35 * person_surround(uv, r * 1.8) - centre);
    let pulse = 0.88 + 0.12 * sin(frame.clip_time * params.speed * 6.283185307);
    let amount = params.amount * 0.01 * pulse;
    let colour = person_tint(params.hue);
    return person_lit(base, amount * (core * (colour * 1.6 + vec3<f32>(0.6)) + glow * colour * 0.65),
        (core + glow * 0.5) * amount);
}
