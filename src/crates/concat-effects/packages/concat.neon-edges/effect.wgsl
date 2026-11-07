// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

struct Params { amount: f32, width: f32, background: f32 }
fn light(uv: vec2<f32>) -> f32 {
    let c = sample_premultiplied(uv);
    return luma(to_display(c.rgb));
}
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    if (params.amount <= 0.0) { return c; }
    let d = vec2<f32>(params.width) / max(frame.size, vec2<f32>(1.0));
    let tl = light(uv + d * vec2<f32>(-1.0, -1.0));
    let tc = light(uv + d * vec2<f32>(0.0, -1.0));
    let tr = light(uv + d * vec2<f32>(1.0, -1.0));
    let ml = light(uv + d * vec2<f32>(-1.0, 0.0));
    let mr = light(uv + d * vec2<f32>(1.0, 0.0));
    let bl = light(uv + d * vec2<f32>(-1.0, 1.0));
    let bc = light(uv + d * vec2<f32>(0.0, 1.0));
    let br = light(uv + d * vec2<f32>(1.0, 1.0));
    let gx = -tl - 2.0*ml - bl + tr + 2.0*mr + br;
    let gy = -tl - 2.0*tc - tr + bl + 2.0*bc + br;
    let edge = length(vec2<f32>(gx, gy));
    let tint = mix(vec3<f32>(0.05, 1.3, 1.8), vec3<f32>(1.5, 0.08, 1.1), abs(gx) / max(abs(gx) + abs(gy), 1e-6));
    let neon = c.rgb * params.background * 0.01 + tint * edge;
    return vec4<f32>(mix(c.rgb, neon, params.amount * 0.01), c.a);
}
