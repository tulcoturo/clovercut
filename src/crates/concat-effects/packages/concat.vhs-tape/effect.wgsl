// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

struct Params { amount: f32, tracking: f32, speed: f32 }
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let base = sample(uv);
    if (params.amount <= 0.0) { return base; }
    let strength = params.amount * 0.01;
    let t = frame.clip_time * params.speed;
    let band = exp(-pow((uv.y - fract(t * 0.13 + 0.83)) / 0.035, 2.0));
    let wobble = sin(uv.y * 90.0 + t * 11.0) * 0.0015 + band * sin(t * 8.0 + uv.y * 350.0) * 0.018 * params.tracking * 0.01;
    let q = clamp(uv + vec2<f32>(wobble * strength, 0.0), vec2<f32>(0.0), vec2<f32>(1.0));
    let c = sample_premultiplied(q);
    let bleed = vec2<f32>(0.0035 * strength, 0.0);
    let a = sample_premultiplied(q - bleed);
    let b = sample_premultiplied(q + bleed);
    let soft = (a + c * 2.0 + b) * 0.25;
    let grey = luma(soft.rgb / max(soft.a, 1e-6));
    let chroma = vec3<f32>(soft.r / max(soft.a, 1e-6), c.g / max(c.a, 1e-6), b.b / max(b.a, 1e-6));
    let colour = mix(chroma, vec3<f32>(grey), 0.12 * strength);
    let lines = 1.0 - strength * 0.08 * (0.5 + 0.5 * cos(uv.y * frame.size.y * 3.14159265));
    return vec4<f32>(colour * lines, c.a);
}
