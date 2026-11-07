// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

struct Params { amount: f32, spacing: f32, softness: f32 }
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    let phase = uv.y * frame.size.y / params.spacing;
    let line = 0.5 + 0.5 * cos(phase * 6.2831853);
    let shape = pow(line, mix(8.0, 1.0, params.softness * 0.01));
    // Fade subpixel stripes instead of aliasing as the canvas gets smaller.
    let visible = smoothstep(1.0, 2.0, params.spacing);
    return vec4<f32>(c.rgb * (1.0 - shape * params.amount * 0.01 * visible), c.a);
}
