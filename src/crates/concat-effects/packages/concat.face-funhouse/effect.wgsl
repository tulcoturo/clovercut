struct Params { amount: f32, frequency: f32, speed: f32, radius: f32 }

fn warp_funhouse(source: vec2<f32>, face: TrackedFace) -> vec2<f32> {
    if (face.mouth_score.z < 0.7) { return vec2<f32>(0.0); }
    let aspect = vec2<f32>(face.mouth_score.w, 1.0);
    let eyes_mid = (face.eyes.xy + face.eyes.zw) * 0.5;
    let eye_vec = (face.eyes.zw - face.eyes.xy) * aspect;
    let eye_dist = max(length(eye_vec), 0.0001);

    let face_center = eyes_mid;
    let face_r = eye_dist * 2.1 * (params.radius * 0.01);

    let d = (source - face_center) * aspect;
    let dist = length(d);
    if (dist >= face_r) { return vec2<f32>(0.0); }

    let t = dist / face_r;
    let falloff = (1.0 - t * t) * (1.0 - t * t);

    let time = frame.clip_time * params.speed * 4.0;
    let wave_freq = params.frequency * 6.283185307 / eye_dist;

    let wave_x = sin(d.y * wave_freq + time) * cos(d.x * wave_freq * 0.5);
    let wave_y = cos(d.x * wave_freq - time) * sin(d.y * wave_freq * 0.5);

    let strength = params.amount * 0.00035 * eye_dist * falloff;
    let shift_source = vec2<f32>(wave_x, wave_y) * strength / aspect;

    return shift_source;
}

fn effect(uv: vec2<f32>) -> vec4<f32> {
    if (params.amount <= 0.0) { return sample(uv); }
    let source = face_source(uv);

    var shift = vec2<f32>(0.0);
    shift += warp_funhouse(source, face_inputs.faces[0]);
    shift += warp_funhouse(source, face_inputs.faces[1]);
    shift += warp_funhouse(source, face_inputs.faces[2]);
    shift += warp_funhouse(source, face_inputs.faces[3]);

    return sample(uv + shift);
}
