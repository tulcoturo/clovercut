struct Params { progress: f32, direction: f32, spread: f32, hue: f32 }

fn ember_tint(hue: f32) -> vec3<f32> {
    let p = abs(fract(vec3<f32>(hue / 360.0) + vec3<f32>(0.0, 0.6666667, 0.3333333)) * 6.0 - 3.0);
    return mix(vec3<f32>(0.08), clamp(p - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(0.92));
}

fn dis_hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(123.34, 456.21));
    let v = q + dot(q, q + vec2<f32>(45.32));
    return fract(v.x * v.y);
}

fn dis_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (vec2<f32>(3.0) - 2.0 * f);
    return mix(
        mix(dis_hash(i), dis_hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(dis_hash(i + vec2<f32>(0.0, 1.0)), dis_hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let base = sample(uv);
    let person = subject_at(uv);
    let progress = clamp(params.progress * 0.01, 0.0, 1.0);
    let aspect = vec2<f32>(frame.size.x / frame.size.y, 1.0);
    let p = uv * aspect;
    let rad = params.direction * 0.0174532925;
    let wind = vec2<f32>(cos(rad), sin(rad));
    let wind_step = wind * (params.spread * 0.005 * progress) / aspect;
    // Read both mattes before deciding coverage, including on WARP/FXC.
    let sample_prev = subject_at(uv - wind_step);

    // Normalise the directional sweep across the frame for every wind angle
    // and aspect ratio, so 0% is untouched and 100% removes the whole person.
    let low = dot(min(wind, vec2<f32>(0.0)), aspect);
    let extent = dot(abs(wind), aspect);
    let flow = clamp((dot(p, wind) - low) / max(extent, 0.0001), 0.0, 1.0);
    let n = dis_noise(p * 22.0 + vec2<f32>(frame.clip_time * 0.4));
    let metric = flow * 0.3 + n * 0.7;
    let burn = metric - (progress * 1.2 - 0.1);
    let dissolved = 1.0 - smoothstep(-0.035, 0.0, burn);
    let life = 4.0 * progress * (1.0 - progress);
    let ember_col = ember_tint(params.hue);
    let edge = (1.0 - smoothstep(0.0, 0.09, abs(burn))) * person * life;
    let glow = (ember_col * 3.5 + vec3<f32>(1.0, 0.9, 0.7)) * edge;

    // Remove only the matte-covered share. Outside it, the source retains
    // its own coverage; a soft matte gives a correspondingly soft cutout.
    let remaining = base.a * (1.0 - person * dissolved);
    let spark_noise = dis_noise(p * 75.0 - wind * frame.clip_time * 3.0);
    let spark = smoothstep(0.88, 1.0, spark_noise) * sample_prev
        * (1.0 - person) * life * base.a;
    let spark_col = ember_col * 4.0 + vec3<f32>(1.0, 0.9, 0.6);
    let alpha = spark + remaining * (1.0 - spark);
    let premultiplied = spark_col * spark
        + (base.rgb + glow) * remaining * (1.0 - spark);
    return vec4<f32>(premultiplied / max(alpha, 0.000001), alpha);
}
