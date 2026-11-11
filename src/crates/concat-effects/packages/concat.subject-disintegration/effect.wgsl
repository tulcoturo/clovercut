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

    let progress = params.progress * 0.01;
    if (progress <= 0.0) { return base; }

    let p = uv * frame.size / frame.size.y;
    let rad = params.direction * 0.0174532925;
    let wind = vec2<f32>(cos(rad), sin(rad));

    let flight_dist = params.spread * 0.01 * progress;
    let wind_step = wind * (flight_dist * 0.5);
    let sample_prev = subject_at(uv - wind_step);

    // If no subject here and none upstream, leave original base completely untouched
    if (person <= 0.001 && sample_prev <= 0.001) {
        return base;
    }

    let flow_pos = dot(p, wind) * 1.5;
    let n = dis_noise(p * 22.0 + vec2<f32>(frame.clip_time * 0.4));
    let dissolve_metric = flow_pos * 0.3 + n * 0.7;

    let threshold = progress * 1.3 - 0.15;
    let burn = dissolve_metric - threshold;

    let ember_col = ember_tint(params.hue);

    var result_rgb = base.rgb;
    var result_a = base.a;

    if (person > 0.01) {
        if (burn < 0.0) {
            result_a = 0.0;
        } else if (burn < 0.09) {
            let edge_t = 1.0 - (burn / 0.09);
            let glow = ember_col * (edge_t * 3.5 + 0.5) + vec3<f32>(1.0, 0.9, 0.7) * (edge_t * edge_t * 2.0);
            result_rgb = mix(base.rgb, glow, edge_t);
            result_a = base.a * person;
        } else {
            result_a = base.a * person;
        }
    } else {
        result_a = 0.0;
    }

    if (sample_prev > 0.1 && (1.0 - person) > 0.01) {
        let p_spark = dis_noise(p * 75.0 - wind * frame.clip_time * 3.0);
        if (p_spark > 0.88) {
            let spark_int = (p_spark - 0.88) / 0.12;
            let spark_col = ember_col * (spark_int * 4.0) + vec3<f32>(1.0, 0.9, 0.6) * spark_int;
            result_rgb += spark_col * sample_prev * (1.0 - person);
            result_a = max(result_a, spark_int * sample_prev);
        }
    }

    let comp_a = clamp(result_a, 0.0, 1.0);
    return vec4<f32>(mix(base.rgb, result_rgb, comp_a), base.a);
}
