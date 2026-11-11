struct Params { amount: f32, width: f32, speed: f32, hue: f32 }

fn runner_tint(hue: f32) -> vec3<f32> {
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

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let base = sample(uv);
    if (params.amount <= 0.0) { return base; }

    let r = params.width * 0.01;
    let s_outer = person_surround(uv, r * 1.8);
    if (s_outer <= 0.001) { return base; }

    let centre = subject_at(uv);
    let s_inner = person_surround(uv, r * 0.4);

    let edge = max(0.0, s_inner - centre);
    let edge_halo = max(0.0, s_outer - centre);

    let p = (uv - vec2<f32>(0.5)) * frame.size / frame.size.y;
    let angle = atan2(p.y, p.x) / 6.283185307 + 0.5;

    let t = frame.clip_time * params.speed;

    let runner1 = pow(fract(angle * 2.0 - t * 1.5), 8.0);
    let runner2 = pow(fract(angle * 2.0 + 0.5 + t * 1.8), 8.0);

    let spark = max(runner1, runner2);

    let col_main = runner_tint(params.hue);
    let col_trail = runner_tint(params.hue + 45.0);

    let amount = params.amount * 0.01;
    let bright_spark = spark * edge * 4.0;
    let outline = edge * 0.7 + edge_halo * 0.3;

    let emission = amount * (mix(col_trail, col_main, spark) * outline + vec3<f32>(0.9, 0.95, 1.0) * bright_spark);
    let cover = amount * (edge + edge_halo * 0.2 + spark * 0.5);

    return person_lit(base, emission, cover);
}
