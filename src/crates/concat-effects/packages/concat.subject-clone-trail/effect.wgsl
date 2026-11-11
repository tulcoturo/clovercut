struct Params { amount: f32, spread: f32, speed: f32, glow: f32 }

fn person_lit(base: vec4<f32>, emission: vec3<f32>, cover: f32) -> vec4<f32> {
    let alpha = max(base.a, clamp(cover, 0.0, 1.0));
    if (alpha <= 0.000001) { return base; }
    return vec4<f32>((base.rgb * base.a + emission) / alpha, alpha);
}

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let base = sample(uv);
    if (params.amount <= 0.0) { return base; }

    let t = frame.clip_time * params.speed * 3.14159265;
    let spread = params.spread * 0.01;

    let off1 = vec2<f32>(cos(t), sin(t * 0.8) * 0.35) * spread;
    let off2 = vec2<f32>(-cos(t * 0.85), -sin(t * 0.75) * 0.35) * spread;
    let off3 = vec2<f32>(sin(t * 1.2) * 0.5, cos(t) * 0.4) * (spread * 0.6);

    let m_center = subject_at(uv);
    let m1 = subject_at(uv - off1);
    let m2 = subject_at(uv - off2);
    let m3 = subject_at(uv - off3);

    let total_ghost = max(m1, max(m2, m3));
    if (total_ghost <= 0.001 && m_center <= 0.001) { return base; }

    let behind = max(0.0, 1.0 - m_center * 0.95);

    let col1 = vec3<f32>(0.1, 0.8, 1.0);
    let col2 = vec3<f32>(1.0, 0.15, 0.65);
    let col3 = vec3<f32>(1.0, 0.85, 0.1);

    let samp1 = sample(uv - off1).rgb;
    let samp2 = sample(uv - off2).rgb;

    let glow_boost = 1.0 + params.glow * 0.015;
    let ghost1 = m1 * (samp1 * 0.4 + col1 * 0.8) * glow_boost;
    let ghost2 = m2 * (samp2 * 0.4 + col2 * 0.8) * glow_boost;
    let ghost3 = m3 * col3 * 0.6 * glow_boost;

    let trail_emission = (ghost1 + ghost2 + ghost3) * behind * (params.amount * 0.01);
    let trail_cover = (m1 * 0.5 + m2 * 0.5 + m3 * 0.3) * behind * (params.amount * 0.01);

    return person_lit(base, trail_emission, trail_cover);
}
