struct Params { amount: f32, eye_size: f32, chin_pinch: f32 }

fn warp_alien(source: vec2<f32>, face: TrackedFace) -> vec2<f32> {
    if (face.mouth_score.z < 0.7) { return vec2<f32>(0.0); }
    let aspect = vec2<f32>(face.mouth_score.w, 1.0);
    let eye_vec = (face.eyes.zw - face.eyes.xy) * aspect;
    let eye_dist = max(length(eye_vec), 0.0001);
    let dir_x = eye_vec / eye_dist;
    let dir_down = vec2<f32>(dir_x.y, -dir_x.x);

    var shift = vec2<f32>(0.0);

    let eye_r = eye_dist * 0.48 * (params.eye_size * 0.01);
    let left_center = face.eyes.xy;
    let right_center = face.eyes.zw;

    let delta_l = (source - left_center) * aspect;
    let dist_l = length(delta_l);
    if (dist_l < eye_r) {
        let t = dist_l / eye_r;
        let pull = (1.0 - t * t) * (params.amount * 0.0045);
        shift += ((delta_l * (1.0 - pull)) / aspect) - (source - left_center);
    }

    let delta_r = (source - right_center) * aspect;
    let dist_r = length(delta_r);
    if (dist_r < eye_r) {
        let t = dist_r / eye_r;
        let pull = (1.0 - t * t) * (params.amount * 0.0045);
        shift += ((delta_r * (1.0 - pull)) / aspect) - (source - right_center);
    }

    let chin_pos = face.nose_mouth.zw + dir_down * eye_dist * 0.3;
    let delta_c = (source - chin_pos) * aspect;
    let dist_c = length(delta_c);
    let chin_r = eye_dist * 0.95;
    if (dist_c < chin_r) {
        let t = dist_c / chin_r;
        let pinch = (1.0 - t * t) * (params.chin_pinch * params.amount * 0.00004);
        let lateral = dot(delta_c, dir_x);
        shift += (dir_x * lateral * pinch) / aspect;
    }

    return shift;
}

fn effect(uv: vec2<f32>) -> vec4<f32> {
    if (params.amount <= 0.0) { return sample(uv); }
    let source = face_source(uv);
    var shift = vec2<f32>(0.0);
    shift += warp_alien(source, face_inputs.faces[0]);
    shift += warp_alien(source, face_inputs.faces[1]);
    shift += warp_alien(source, face_inputs.faces[2]);
    shift += warp_alien(source, face_inputs.faces[3]);
    return sample(uv + shift);
}
