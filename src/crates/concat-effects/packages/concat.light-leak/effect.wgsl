// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

struct Params { amount: f32, position: f32, width: f32, speed: f32 }
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    if (c.a <= 0.0) { return vec4<f32>(0.0); }
    let t = frame.clip_time * params.speed;
    let centre = params.position * 0.01 + sin(t * 1.3) * 0.12;
    let width = max(params.width * 0.01, 0.05);
    let stripe = exp(-pow((uv.x - centre) / width, 2.0) * 3.0);
    let falloff = 0.55 + 0.45 * sin(uv.y * 2.8 + t + 0.5);
    let strength = stripe * falloff * params.amount * 0.01;
    // Veiling flare lifts dark footage too, without adding any alpha coverage.
    let tint = vec3<f32>(1.8, 0.55, 0.12);
    let reflected = max(luma(c.rgb), 0.0) * tint;
    return vec4<f32>(c.rgb + (reflected * 1.5 + tint * 0.3) * strength, c.a);
}
