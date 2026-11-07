// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

struct Params { amount: f32, speed: f32, blocks: f32 }
fn noise(n: f32) -> f32 { return fract(sin(n * 12.9898 + 78.233) * 43758.5453); }
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let base = sample(uv);
    if (params.amount <= 0.0) { return base; }
    let tick = floor(frame.clip_time * params.speed);
    let row = floor(uv.y * params.blocks);
    let seed = row + tick * 97.0;
    let gate = smoothstep(0.55, 0.8, noise(seed));
    let strength = params.amount * 0.01;
    let shift = (noise(seed + 21.0) - 0.5) * 0.3 * strength * gate;
    let q = vec2<f32>(fract(uv.x + shift + 1.0), uv.y);
    let c = sample_premultiplied(q);
    let delta = vec2<f32>(0.012 * strength * gate, 0.0);
    let r = sample_premultiplied(vec2<f32>(fract(q.x + delta.x), q.y));
    let b = sample_premultiplied(vec2<f32>(fract(q.x - delta.x + 1.0), q.y));
    return vec4<f32>(r.r / max(r.a, 1e-6), c.g / max(c.a, 1e-6), b.b / max(b.a, 1e-6), c.a);
}
