// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

struct Params { amount: f32, position: f32, width: f32, rotation: f32 }
fn effect(uv: vec2<f32>) -> vec4<f32> {
    if (params.amount <= 0.0) { return sample(uv); }
    let aspect = frame.size.x / max(frame.size.y, 1.0);
    let angle = radians(params.rotation);
    let p = (uv - vec2<f32>(0.5, params.position * 0.01)) * vec2<f32>(aspect, 1.0);
    let distance = abs(dot(p, vec2<f32>(sin(angle), cos(angle))));
    let focus = params.width * 0.005;
    let defocus = smoothstep(focus, focus + 0.18, distance);
    let radius = params.amount * 0.01 * defocus * 0.025;
    if (radius < 0.00001) { return sample(uv); }
    // Fixed-cost concentric aperture samples, accumulated premultiplied.
    var total = sample_premultiplied(uv);
    for (var i = 0; i < 32; i++) {
        let r = sqrt((f32(i) + 0.5) / 32.0);
        let a = f32(i) * 2.39996323;
        let delta = vec2<f32>(cos(a) / aspect, sin(a)) * r * radius;
        total += sample_premultiplied(uv + delta);
    }
    let c = total / 33.0;
    return vec4<f32>(c.rgb / max(c.a, 1e-6), c.a);
}
