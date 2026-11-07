// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

struct Params { amount: f32, grain: f32, wear: f32, speed: f32 }
fn film_noise(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}
fn effect(uv: vec2<f32>) -> vec4<f32> {
    let c = sample(uv);
    if (params.amount <= 0.0 || c.a <= 0.0) { return c; }
    let tick = floor(frame.clip_time * params.speed);
    let strength = params.amount * 0.01;
    let wear = params.wear * 0.01;
    let grey = max(luma(c.rgb), 0.0);
    let tint = vec3<f32>(1.12, 1.0, 0.78);
    let flicker = 1.0 + (film_noise(vec2<f32>(tick, 3.0)) - 0.5) * wear * 0.18;
    var aged = grey * tint * flicker;
    // Multiplicative grain keeps black and extended highlights well defined.
    let pixel = floor(uv * frame.size);
    let grain = (film_noise(pixel + vec2<f32>(tick * 17.0, tick * 31.0)) - 0.5) * params.grain * 0.006;
    aged *= 1.0 + grain;
    var damage = 1.0;
    for (var i = 0; i < 3; i++) {
        let seed = vec2<f32>(floor(tick / 5.0), f32(i) + 11.0);
        let x = film_noise(seed);
        let thin = max(0.0006, 0.65 / frame.size.x);
        let scratch = exp(-pow((uv.x - x) / thin, 2.0));
        let broken = smoothstep(0.35, 0.65, film_noise(vec2<f32>(floor(uv.y * 80.0) + tick, f32(i))));
        damage *= 1.0 - scratch * broken * wear * 0.85;
    }
    let cell = floor(uv * vec2<f32>(36.0, 24.0));
    let dust = film_noise(cell + vec2<f32>(tick * 5.0, tick * 7.0));
    let spot = 1.0 - smoothstep(0.1, 0.3, length(fract(uv * vec2<f32>(36.0, 24.0)) - vec2<f32>(0.5)));
    damage *= 1.0 - step(0.985, dust) * spot * wear;
    let result = mix(c.rgb, aged * damage, strength);
    return vec4<f32>(result, c.a);
}
