// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

struct Params { amount: f32, speed: f32, rotation: f32 }
fn effect(uv: vec2<f32>) -> vec4<f32> {
    if (params.amount <= 0.0) { return sample(uv); }
    let strength = params.amount * 0.01;
    let t = frame.clip_time * params.speed * 6.2831853;
    let angle = radians(params.rotation) * strength * (0.65 * sin(t * 0.71) + 0.35 * sin(t * 1.93));
    let shift = 0.025 * strength * vec2<f32>(0.65 * sin(t) + 0.35 * sin(t * 2.37), 0.65 * sin(t * 1.31) + 0.35 * sin(t * 2.83));
    let aspect = frame.size.x / max(frame.size.y, 1.0);
    // Bound rotation and translation for every time, avoiding a breathing crop.
    let turn = radians(params.rotation) * strength;
    let crop = 1.0 + sin(turn) * max(aspect, 1.0 / aspect) + 0.05 * strength;
    let p = (uv - vec2<f32>(0.5)) / crop;
    let q = vec2<f32>(cos(angle)*p.x - sin(angle)*p.y/aspect, sin(angle)*p.x*aspect + cos(angle)*p.y);
    let c = sample_premultiplied(vec2<f32>(0.5) + q + shift / crop);
    return vec4<f32>(c.rgb / max(c.a, 1e-6), c.a);
}
