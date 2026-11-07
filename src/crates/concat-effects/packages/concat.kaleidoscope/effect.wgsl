// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

struct Params { amount: f32, segments: f32, rotation: f32, zoom: f32 }
fn effect(uv: vec2<f32>) -> vec4<f32> {
    if (params.amount <= 0.0) { return sample(uv); }
    let aspect = frame.size.x / max(frame.size.y, 1.0);
    let p = (uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0);
    let sector = 6.2831853 / max(round(params.segments), 2.0);
    let theta = atan2(p.y, p.x) + radians(params.rotation);
    let angle = abs(fract(theta / sector + 0.5) - 0.5) * sector;
    let radius = length(p) * 100.0 / params.zoom;
    let q = vec2<f32>(0.5) + radius * vec2<f32>(cos(angle) / aspect, sin(angle));
    // Reflect, rather than stretch, any samples outside the image.
    let mirror = vec2<f32>(1.0) - abs(vec2<f32>(1.0) - (q - floor(q * 0.5) * 2.0));
    let c = sample_premultiplied(mirror);
    let original = sample_premultiplied(uv);
    let out = mix(original, c, params.amount * 0.01);
    return vec4<f32>(out.rgb / max(out.a, 1e-6), out.a);
}
