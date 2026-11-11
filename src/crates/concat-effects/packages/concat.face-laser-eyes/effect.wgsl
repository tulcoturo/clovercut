struct Params { amount: f32, beam_length: f32, flare_size: f32, hue: f32 }

fn laser_tint(hue: f32) -> vec3<f32> {
    let p = abs(fract(vec3<f32>(hue / 360.0) + vec3<f32>(0.0, 0.6666667, 0.3333333)) * 6.0 - 3.0);
    return mix(vec3<f32>(0.08), clamp(p - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(0.92));
}

fn draw_laser_eye(source: vec2<f32>, eye_pos: vec2<f32>, eye_dist: f32, dir_beam: vec2<f32>, aspect: vec2<f32>, laser_col: vec3<f32>) -> vec3<f32> {
    let d = (source - eye_pos) * aspect;
    let dist = length(d);

    let flare_r = eye_dist * 0.45 * (params.flare_size * 0.01);
    let flare_core = exp(-dist * 2.8 / max(flare_r, 0.001));

    let spike_x = exp(-abs(d.x) * 45.0 / eye_dist) * exp(-abs(d.y) * 4.0 / eye_dist);
    let spike_y = exp(-abs(d.y) * 45.0 / eye_dist) * exp(-abs(d.x) * 4.0 / eye_dist);
    let star = (spike_x + spike_y) * 1.5;

    let along = dot(d, dir_beam);
    let perp = abs(dot(d, vec2<f32>(-dir_beam.y, dir_beam.x)));
    let max_len = eye_dist * 5.0 * (params.beam_length * 0.01);

    var beam = 0.0;
    if (along > 0.0 && along < max_len) {
        let beam_fade = 1.0 - (along / max_len);
        let beam_w = eye_dist * (0.04 + along * 0.02);
        let beam_profile = exp(-perp * perp / max(beam_w * beam_w * 0.5, 0.00001));
        beam = beam_profile * beam_fade * 2.0;
    }

    let glow = (flare_core * 2.5 + star + beam) * (params.amount * 0.01);
    return laser_col * glow + vec3<f32>(1.0, 0.95, 0.9) * (flare_core * 3.0 + beam * 1.5) * (params.amount * 0.01);
}

fn decorate_face_lasers(source: vec2<f32>, face: TrackedFace, laser_col: vec3<f32>) -> vec3<f32> {
    if (face.mouth_score.z < 0.7) { return vec3<f32>(0.0); }
    let aspect = vec2<f32>(face.mouth_score.w, 1.0);
    let eye_vec = (face.eyes.zw - face.eyes.xy) * aspect;
    let eye_dist = max(length(eye_vec), 0.0001);
    let dir_x = eye_vec / eye_dist;
    let dir_beam = vec2<f32>(dir_x.y, -dir_x.x);

    let left = draw_laser_eye(source, face.eyes.xy, eye_dist, dir_beam, aspect, laser_col);
    let right = draw_laser_eye(source, face.eyes.zw, eye_dist, dir_beam, aspect, laser_col);
    return left + right;
}

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let base = sample(uv);
    if (params.amount <= 0.0) { return base; }
    let source = face_source(uv);
    let laser_col = laser_tint(params.hue);

    var emission = vec3<f32>(0.0);
    emission += decorate_face_lasers(source, face_inputs.faces[0], laser_col);
    emission += decorate_face_lasers(source, face_inputs.faces[1], laser_col);
    emission += decorate_face_lasers(source, face_inputs.faces[2], laser_col);
    emission += decorate_face_lasers(source, face_inputs.faces[3], laser_col);

    if (length(emission) <= 0.0001) { return base; }

    return vec4<f32>(base.rgb + emission, base.a);
}
