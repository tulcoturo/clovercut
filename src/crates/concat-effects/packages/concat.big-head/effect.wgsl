struct Params { amount: f32, radius: f32, feather: f32 }

fn warp_face(source: vec2<f32>, face: TrackedFace) -> vec2<f32> {
    if (face.mouth_score.z < 0.7) { return vec2<f32>(0.0); }
    let aspect = vec2<f32>(face.mouth_score.w, 1.0);
    let eyes_mid = (face.eyes.xy + face.eyes.zw) * 0.5;
    let eye_vec = (face.eyes.zw - face.eyes.xy) * aspect;
    let eye_dist = max(length(eye_vec), 0.0001);
    let dir_x = eye_vec / eye_dist;
    let dir_up = vec2<f32>(-dir_x.y, dir_x.x);
    let head_center = (eyes_mid * aspect - dir_up * eye_dist * 0.35) / aspect;
    let head_r = eye_dist * 1.85 * (params.radius * 0.01);
    let delta = (source - head_center) * aspect;
    let dist = length(delta);
    if (dist >= head_r) { return vec2<f32>(0.0); }
    let norm_dist = dist / head_r;
    let falloff = 1.0 - smoothstep(1.0 - params.feather * 0.01, 1.0, norm_dist);
    let strength = params.amount * 0.0065 * falloff;
    let pull = (1.0 - norm_dist * norm_dist) * strength;
    let sample_src = head_center + ((delta * (1.0 - pull)) / aspect);
    return sample_src - source;
}

fn effect(uv: vec2<f32>) -> vec4<f32> {
    if (params.amount <= 0.0) { return sample(uv); }
    let source = face_source(uv);
    var shift = vec2<f32>(0.0);
    shift += warp_face(source, face_inputs.faces[0]);
    shift += warp_face(source, face_inputs.faces[1]);
    shift += warp_face(source, face_inputs.faces[2]);
    shift += warp_face(source, face_inputs.faces[3]);
    return sample(uv + shift);
}
