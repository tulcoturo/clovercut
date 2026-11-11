struct Params { amount: f32, width: f32, hue: f32, speed: f32 }

fn aura_tint(hue: f32) -> vec3<f32> {
    let p = abs(fract(vec3<f32>(hue / 360.0) + vec3<f32>(0.0, 0.6666667, 0.3333333)) * 6.0 - 3.0);
    return mix(vec3<f32>(0.08), clamp(p - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(0.92));
}

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
    return vec4<f32>((base.rgb * base.a + emission) / alpha, alpha);
}

fn aura_hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(123.34, 456.21));
    let v = q + dot(q, q + vec2<f32>(45.32));
    return fract(v.x * v.y);
}

fn aura_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (vec2<f32>(3.0) - 2.0 * f);
    return mix(mix(aura_hash(i), aura_hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(aura_hash(i + vec2<f32>(0.0, 1.0)), aura_hash(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let base = sample(uv);
    if (params.amount <= 0.0) { return base; }
    let centre = subject_at(uv);
    let r = params.width * 0.01;
    let s_outer = person_surround(uv, r * 2.2);
    if (s_outer <= 0.001) { return base; }

    let s_inner = person_surround(uv, r * 0.35);
    let s_mid = person_surround(uv, r * 1.1);

    let core = max(0.0, s_inner - centre);
    let mid_band = max(0.0, s_mid - centre);
    let outer_band = max(0.0, s_outer - centre);

    let t = frame.clip_time * params.speed;
    let p = uv * frame.size / frame.size.y;

    let wave = 0.5 + 0.5 * sin(outer_band * 35.0 - t * 8.0);
    let noise_val = aura_noise(p * 40.0 + vec2<f32>(t * 1.5, -t * 3.0));

    let col_primary = aura_tint(params.hue);
    let col_secondary = aura_tint(params.hue + 55.0);

    let energy = (core * 2.0 + mid_band * wave * 1.5 + outer_band * noise_val * 0.7);
    let amount = params.amount * 0.01;
    let emission = amount * (mix(col_primary, col_secondary, wave) * energy + vec3<f32>(0.7) * core);
    let cover = amount * (core + mid_band * 0.6 + outer_band * 0.3);

    return person_lit(base, emission, cover);
}
