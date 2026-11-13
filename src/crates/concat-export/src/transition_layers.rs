// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Clovercut contributors

//! Movable transition layers, adapted independently of legacy cut rendering.
use super::*;

struct HeldImage {
    source: GeneratedImage,
    at: f64,
}
impl ImageSource for HeldImage {
    fn frame(&self, _at: f64, width: u32, height: u32) -> Result<Arc<Frame>, String> {
        self.source.0.frame(self.at, width, height)
    }
}

pub(super) fn resolve(
    clips: &mut Vec<ExportClip>,
    rate: FrameRate,
    origin: f64,
) -> Vec<TransitionSpan> {
    let frame = 1.0 / rate.fps().as_f64();
    on_their_own_lanes(clips, frame);

    let mut spans: Vec<TransitionSpan> = Vec::new();
    let mut next_lane = clips.iter().map(|clip| clip.track).max().unwrap_or(0) + 1;
    let transitions: Vec<usize> = (0..clips.len())
        .filter(|&at| clips[at].kind == ClipKind::Transition && !clips[at].hidden)
        .collect();
    for at in transitions {
        let transition = &clips[at];
        let Some(spec) = transition.transition.clone() else {
            continue;
        };
        if transition.duration < frame / 2.0 {
            continue;
        }
        let (start, end, lane) = (
            transition.start,
            transition.start + transition.duration,
            transition.track,
        );
        let span = spans.len();
        if let Some((outgoing, incoming)) = cut_under(clips, start, end, lane, frame) {
            let cut = clips[incoming].start;
            let outgoing_lane = clips[outgoing].track;
            clips[outgoing].roles.push((span, false));
            clips[incoming].roles.push((span, true));
            if end - cut > frame / 2.0 {
                let mut after = running_on(&clips[outgoing], cut, end - cut, frame);
                stand_in(&mut after, outgoing_lane, next_lane, span, false);
                next_lane += 1;
                clips.push(after);
            }
            if cut - start > frame / 2.0 {
                let mut before = coming_early(&clips[incoming], start, cut - start, frame);
                stand_in(&mut before, outgoing_lane, next_lane, span, true);
                next_lane += 1;
                clips.push(before);
            }
        }
        let first = quantise_at(start, rate, origin);
        spans.push(TransitionSpan {
            start: first,
            // Its end rounded as a clip's is, so it ends on the frame the
            // clips it runs over do.
            end: quantise_at(end, rate, origin),
            to_track: lane,
            layered: true,
            id: spec.kind,
            params: spec.params,
            from: std::collections::HashSet::new(),
            to: std::collections::HashSet::new(),
        });
    }
    spans
}

/// Makes each transition still set on a cut the old way - on the clip
/// coming in, `ExportClip::transition` on a picture - a transition clip of
/// its own, centred on the cut and no longer than either side, on a lane
/// just above the cut's. The lanes are doubled first when there are any,
/// so every lane has a free one above it and the stacking against every
/// other lane is kept. A transition on a clip with nothing ending where it
/// starts - the cut edited apart - is let go, as the timeline lets it go.
fn on_their_own_lanes(clips: &mut Vec<ExportClip>, frame: f64) {
    let cuts: Vec<(usize, usize)> = clips
        .iter()
        .enumerate()
        .filter(|(_, clip)| {
            clip.kind != ClipKind::Transition && clip.transition.is_some() && !clip.hidden
        })
        .filter_map(|(incoming, clip)| {
            let outgoing = clips.iter().position(|other| {
                !other.hidden
                    && other.kind.is_visual()
                    && other.track == clip.track
                    && (other.start + other.duration - clip.start).abs() < frame / 2.0
            })?;
            (clip.kind.is_visual() && outgoing != incoming).then_some((outgoing, incoming))
        })
        .collect();
    for clip in clips.iter_mut() {
        if clip.kind != ClipKind::Transition && cuts.is_empty() {
            clip.transition = None;
        }
    }
    if cuts.is_empty() {
        return;
    }
    for clip in clips.iter_mut() {
        clip.track *= 2;
    }
    for (outgoing, incoming) in cuts {
        let Some(spec) = clips[incoming].transition.take() else {
            continue;
        };
        let length = spec
            .duration
            .min(clips[outgoing].duration)
            .min(clips[incoming].duration);
        if length < frame {
            continue;
        }
        let cut = clips[incoming].start;
        let lane = clips[incoming].track + 1;
        clips.push(ExportClip {
            transition: Some(spec),
            ..ExportClip::blank(ClipKind::Transition, cut - length / 2.0, length, lane)
        });
    }
    for clip in clips.iter_mut() {
        if clip.kind != ClipKind::Transition {
            clip.transition = None;
        }
    }
}

/// The cut a transition over `[start, end)` on `lane` carries across, as
/// the outgoing and incoming clips: two pictures meeting on a lane beneath
/// it within its span, give or take half a frame. The one nearest the
/// span's middle where there are several, and of two equally near, the
/// higher lane's - the one on top of the picture.
fn cut_under(
    clips: &[ExportClip],
    start: f64,
    end: f64,
    lane: usize,
    frame: f64,
) -> Option<(usize, usize)> {
    let middle = (start + end) / 2.0;
    let pictures = || {
        clips
            .iter()
            .enumerate()
            .filter(|(_, clip)| clip.kind.is_visual() && !clip.hidden && clip.track < lane)
    };
    pictures()
        .filter_map(|(incoming, clip)| {
            let cut = clip.start;
            if cut < start - frame / 2.0 || cut > end + frame / 2.0 {
                return None;
            }
            let (outgoing, _) = pictures().find(|(at, other)| {
                *at != incoming
                    && other.track == clip.track
                    && (other.start + other.duration - cut).abs() < frame / 2.0
            })?;
            Some((outgoing, incoming))
        })
        .min_by(|&(_, one), &(_, other)| {
            let off = |at: usize| (clips[at].start - middle).abs();
            off(one)
                .total_cmp(&off(other))
                .then(clips[other].track.cmp(&clips[one].track))
        })
}

/// The outgoing clip carried on past the cut for `length` seconds from
/// `cut`: its own footage after its out-point where the file has that
/// much, its last frame held where it has not.
fn running_on(clip: &ExportClip, cut: f64, length: f64, frame: f64) -> ExportClip {
    if tail_handle(clip, frame) + frame / 2.0 < length {
        return last_frame_held(clip, cut, length, frame);
    }
    let mut after = last_frame_held(clip, cut, length, frame);
    after.hold = false;
    after.effect_clock = Some((clip.start, None));
    after.speed = clip.speed;
    if clip.kind != ClipKind::Image {
        after.source_start = clip.source_start + clip.duration * clip.speed;
    }
    after
}

/// The incoming clip started early, `length` seconds before its cut at
/// `start`: its own footage before its in-point where the file has that
/// much, its first frame held where it has not.
fn coming_early(clip: &ExportClip, start: f64, length: f64, frame: f64) -> ExportClip {
    let mut before = held_at(clip, start, length, true);
    if head_handle(clip) + frame / 2.0 >= length {
        before.speed = clip.speed;
        before.effect_clock = Some((clip.start, None));
        if clip.kind != ClipKind::Image {
            before.source_start = (clip.source_start - length * clip.speed).max(0.0);
        }
    } else {
        before.hold = true;
    }
    before
}

/// Makes `clip` a silent stand-in on lane `lane` for the clip on
/// `stacked_at`, in transition `span`'s outgoing picture or, with
/// `incoming`, its incoming one.
fn stand_in(clip: &mut ExportClip, stacked_at: usize, lane: usize, span: usize, incoming: bool) {
    clip.track = lane;
    clip.stack_at = Some(stacked_at);
    clip.roles = vec![(span, incoming)];
}

/// A copy of `clip`'s last frame held from `start` for `length` seconds:
/// silent, unkeyed and unfaded, placed where its keys leave it at its tail.
fn last_frame_held(clip: &ExportClip, start: f64, length: f64, frame: f64) -> ExportClip {
    let mut hold = held_at(clip, start, length, false);
    hold.hold = true;
    if clip.kind != ClipKind::Image {
        let (played, rate) = match clip.retime() {
            Some(curve) => (clip.duration * curve.consumed(1.0), curve.speed_at(1.0)),
            None => (clip.duration * clip.speed, clip.speed),
        };
        hold.source_start = clip.source_start + (played - frame * rate).max(0.0);
    }
    hold
}

/// `clip` placed at `start` for `length` seconds with no sound, no fades,
/// no transitions and no speed of its own, and every property it keys held
/// where its keys leave it at its head (`head`) or its tail: keys are
/// absolute and a keyed property's constant is the neutral value, so the
/// end of each ride becomes the copy's constant.
fn held_at(clip: &ExportClip, start: f64, length: f64, head: bool) -> ExportClip {
    let mut held = clip.clone();
    held.effect_clock = Some((clip.start, Some(if head { 0.0 } else { clip.duration })));
    if let Some(source) = &clip.generated {
        held.generated = Some(GeneratedImage(Arc::new(HeldImage {
            source: source.clone(),
            at: if head { 0.0 } else { 1.0 },
        })));
    }
    held.start = start;
    held.duration = length;
    held.speed = 1.0;
    held.speed_curve.clear();
    held.speed_slopes.clear();
    held.muted = true;
    held.has_audio = Some(false);
    held.volume = 0.0;
    held.fade_in = 0.0;
    held.fade_out = 0.0;
    held.transition = None;
    held.video_fade_in = 0.0;
    held.transition_shapes.clear();
    held.roles.clear();
    let ends: Vec<(String, f64)> = ["scale", "offsetX", "offsetY", "rotation", "opacity"]
        .into_iter()
        .filter_map(|property| {
            let keys = held.animation.iter().filter(|key| key.property == property);
            let end = if head {
                keys.min_by(|a, b| a.at.total_cmp(&b.at))
            } else {
                keys.max_by(|a, b| a.at.total_cmp(&b.at))
            };
            end.map(|key| (property.to_owned(), key.value))
        })
        .collect();
    held.animation.clear();
    for (property, value) in ends {
        match property.as_str() {
            "scale" => held.scale = value,
            "offsetX" => held.offset_x = value,
            "offsetY" => held.offset_y = value,
            "rotation" => held.rotation = value,
            "opacity" => held.opacity = value,
            _ => {}
        }
    }
    // Freeze the boundary pose instead of restarting its motion on the stand-in.
    let at = if head { 0.0 } else { 1.0 };
    if let Some(motion) = clip.motion.as_ref().and_then(|motion| motion.engine()) {
        let pose = motion.pose_at(at * clip.duration, clip.duration);
        held.scale *= pose.scale.max(0.001);
        held.offset_x += pose.dx;
        held.offset_y += pose.dy;
        held.rotation += pose.rotation;
        held.stretch_x *= pose.stretch_x.max(0.001);
        held.stretch_y *= pose.stretch_y.max(0.001);
        held.opacity *= pose.opacity;
    }
    held.motion = None;
    let (opacity, scale, offset_y) = clip
        .caption_motion
        .sample(at * clip.duration, clip.duration);
    held.opacity *= opacity;
    held.scale *= scale;
    held.offset_y += offset_y;
    held.caption_motion = Default::default();
    for mask in &mut held.masks {
        for number in Mask::NUMBERS {
            let value = mask.value_at(number, at);
            mask.set(number, value);
        }
        mask.keys.clear();
    }
    for effect in &mut held.effects {
        let keys: Vec<_> = effect.keys.keys().cloned().collect();
        for key in keys {
            let fallback = effect.params.get(&key).copied().unwrap_or_default();
            let value = effect.value_at(&key, at, fallback);
            effect.params.insert(key, value);
        }
        effect.keys.clear();
    }
    held
}

pub(super) fn transition_stages(plan: &FramePlan, span: &TransitionSpan) -> (FramePlan, FramePlan) {
    let stage = |leave_out: &std::collections::HashSet<concat_core::timeline::ClipId>| {
        let mut layers: Vec<PlannedLayer> = plan
            .layers
            .iter()
            .filter(|layer| layer.track < span.to_track && !leave_out.contains(&layer.clip))
            .cloned()
            .collect();
        layers.sort_by_key(|layer| layer.track);
        FramePlan {
            time: plan.time,
            width: plan.width,
            height: plan.height,
            canvas: plan.canvas,
            layers,
            treatments: Vec::new(),
            output: plan.output,
        }
    };
    (stage(&span.to), stage(&span.from))
}

/// The transition's frame where its shader cannot run - no GPU, or a
/// package this build does not know: the two pictures dissolved into each
/// other over the span, and whatever is above its lane drawn over that, as
/// `combine_transition` would.
fn dissolve_transition(
    compositor: &mut dyn Compositor,
    plan: &FramePlan,
    span: &TransitionSpan,
    treatments: &[PlannedTreatment],
) -> Frame {
    let (below, over): (Vec<PlannedTreatment>, Vec<PlannedTreatment>) = treatments
        .iter()
        .cloned()
        .partition(|treatment| treatment.track < span.to_track);
    let (mut from_plan, mut to_plan) = transition_stages(plan, span);
    from_plan.treatments = below.clone();
    to_plan.treatments = below;
    let from = compositor.render(&from_plan);
    let to = compositor.render(&to_plan);
    let mixed = mix(&from, &to, span.progress(plan.time) as f32);
    over_the_lane(compositor, plan, span, mixed, over)
}

/// `amount` of the way from `from` to `to`, pixel by pixel; `from` where
/// the two differ in size or depth, which a compositor never makes.
fn mix(from: &Frame, to: &Frame, amount: f32) -> Frame {
    use concat_core::frame::Depth;
    if from.width() != to.width() || from.height() != to.height() || from.depth() != to.depth() {
        return from.clone();
    }
    let mut out = from.clone();
    let amount = amount.clamp(0.0, 1.0);
    fn blend(a: [f32; 4], b: [f32; 4], amount: f32) -> [f32; 4] {
        let wa = a[3] * (1.0 - amount);
        let wb = b[3] * amount;
        let alpha = wa + wb;
        if alpha <= 0.0 {
            return [0.0; 4];
        }
        [
            (a[0] * wa + b[0] * wb) / alpha,
            (a[1] * wa + b[1] * wb) / alpha,
            (a[2] * wa + b[2] * wb) / alpha,
            alpha,
        ]
    }
    match from.depth() {
        Depth::Eight => {
            for (pixel, other) in out
                .pixels_mut()
                .chunks_exact_mut(4)
                .zip(to.pixels().chunks_exact(4))
            {
                let a = std::array::from_fn(|i| f32::from(pixel[i]));
                let b = std::array::from_fn(|i| f32::from(other[i]));
                for (channel, value) in pixel.iter_mut().zip(blend(a, b, amount)) {
                    *channel = value.round() as u8;
                }
            }
        }
        Depth::Sixteen => {
            for (pixel, other) in out
                .pixels_mut()
                .chunks_exact_mut(8)
                .zip(to.pixels().chunks_exact(8))
            {
                let a = std::array::from_fn(|i| {
                    f32::from(u16::from_le_bytes([pixel[i * 2], pixel[i * 2 + 1]]))
                });
                let b = std::array::from_fn(|i| {
                    f32::from(u16::from_le_bytes([other[i * 2], other[i * 2 + 1]]))
                });
                for (channel, value) in pixel.chunks_exact_mut(2).zip(blend(a, b, amount)) {
                    channel.copy_from_slice(&(value.round() as u16).to_le_bytes());
                }
            }
        }
    }
    out
}

/// `combined` with everything above the transition's lane drawn over it,
/// and the treatments above it grading the lot.
fn over_the_lane(
    compositor: &mut dyn Compositor,
    plan: &FramePlan,
    span: &TransitionSpan,
    combined: Frame,
    over: Vec<PlannedTreatment>,
) -> Frame {
    let above: Vec<PlannedLayer> = plan
        .layers
        .iter()
        .filter(|layer| layer.track > span.to_track)
        .cloned()
        .collect();
    if above.is_empty() && over.is_empty() {
        return combined;
    }
    let mut layers = vec![ground_layer(combined)];
    layers.extend(above);
    compositor.render(&FramePlan {
        time: plan.time,
        width: plan.width,
        height: plan.height,
        canvas: plan.canvas,
        layers,
        treatments: over,
        output: plan.output,
    })
}

/// The frame when a transition is live: its two pictures (see
/// `transition_stages`) combined through the package's shader, and what is
/// above its lane over that. `None` when the compositor cannot run the
/// shader or the package is unknown, and the caller dissolves instead.
fn combine_transition(
    compositor: &mut dyn Compositor,
    plan: &FramePlan,
    span: &TransitionSpan,
    treatments: &[PlannedTreatment],
) -> Option<Frame> {
    let pass =
        Catalogue::builtin().transition_pass(&span.id, &span.params, span.progress(plan.time))?;
    // Nothing is ever on the transition's own lane while it runs: the lane
    // is its, so each treatment is either side of it.
    let (below, over): (Vec<PlannedTreatment>, Vec<PlannedTreatment>) = treatments
        .iter()
        .cloned()
        .partition(|treatment| treatment.track < span.to_track);
    let (mut from_plan, mut to_plan) = transition_stages(plan, span);
    from_plan.treatments = below.clone();
    to_plan.treatments = below;
    let from = compositor.render(&from_plan);
    let to = compositor.render(&to_plan);
    let combined = compositor.combine(
        plan.width,
        plan.height,
        plan.time.as_f64() as f32,
        &from,
        &to,
        &pass,
    )?;
    // Anything above the transition's lane draws over the combined
    // picture, and a layer above it grades the lot.
    Some(over_the_lane(compositor, plan, span, combined, over))
}

pub(super) fn composite(
    compositor: &mut dyn Compositor,
    plan: &FramePlan,
    span: &TransitionSpan,
    treatments: &[PlannedTreatment],
) -> Frame {
    combine_transition(compositor, plan, span, treatments)
        .unwrap_or_else(|| dissolve_transition(compositor, plan, span, treatments))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Solid([u8; 4]);
    impl ImageSource for Solid {
        fn frame(&self, _at: f64, width: u32, height: u32) -> Result<Arc<Frame>, String> {
            let mut frame = Frame::black(width, height);
            frame.fill(self.0);
            Ok(Arc::new(frame))
        }
    }
    struct Ramp;
    impl ImageSource for Ramp {
        fn frame(&self, at: f64, width: u32, height: u32) -> Result<Arc<Frame>, String> {
            Solid([(at * 255.0).round() as u8, 0, 0, 255]).frame(at, width, height)
        }
    }

    #[test]
    fn a_stand_in_keeps_the_generated_endpoint_motion_and_mask() {
        use concat_project::model::{ClipLoop, ClipMotion, KeyEase, MaskShape};
        let mut clip = ExportClip::blank(ClipKind::Image, 0.0, 2.0, 0);
        clip.generated = Some(GeneratedImage(Arc::new(Ramp)));
        clip.motion = Some(ClipMotion {
            enter: None,
            exit: None,
            cycle: Some(ClipLoop {
                kind: "pulse".into(),
                seconds: 3.0,
                amount: 1.0,
            }),
        });
        let mut mask = Mask::of("mask", "Mask", MaskShape::Circle);
        mask.set_key("opacity", 0.0, 0.25, KeyEase::LINEAR);
        mask.set_key("opacity", 1.0, 0.75, KeyEase::LINEAR);
        clip.masks.push(mask);
        let expected = clip
            .motion
            .as_ref()
            .unwrap()
            .engine()
            .unwrap()
            .pose_at(2.0, 2.0);
        let tail = held_at(&clip, 2.0, 0.5, false);
        let head = held_at(&clip, 1.5, 0.5, true);
        for at in [0.0, 0.5, 1.0] {
            assert_eq!(
                tail.generated
                    .as_ref()
                    .unwrap()
                    .0
                    .frame(at, 8, 8)
                    .unwrap()
                    .pixel(4, 4),
                Some([255, 0, 0, 255])
            );
            assert_eq!(
                head.generated
                    .as_ref()
                    .unwrap()
                    .0
                    .frame(at, 8, 8)
                    .unwrap()
                    .pixel(4, 4),
                Some([0, 0, 0, 255])
            );
        }
        assert!((tail.scale - expected.scale).abs() < 1e-9);
        assert!(tail.motion.is_none());
        assert!(tail.masks[0].keys.is_empty());
        assert_eq!(tail.masks[0].value_at("opacity", 0.2), 0.75);
        assert_eq!(tail.effect_clock, Some((0.0, Some(2.0))));
    }

    #[test]
    fn a_transition_layer_has_both_pictures_at_the_cut_on_the_cpu() {
        let image = |start, colour| ExportClip {
            generated: Some(GeneratedImage(Arc::new(Solid(colour)))),
            ..ExportClip::blank(ClipKind::Image, start, 2.0, 0)
        };
        let transition = ExportClip {
            transition: Some(TransitionSpec {
                kind: "concat.push".into(),
                duration: 1.0,
                params: Default::default(),
            }),
            ..ExportClip::blank(ClipKind::Transition, 1.5, 1.0, 1)
        };
        let request = PreviewFrameRequest {
            time: 2.0,
            width: 16,
            height: 16,
            rate_num: 30,
            rate_den: 1,
            clips: vec![
                image(0.0, [255, 0, 0, 255]),
                image(2.0, [0, 0, 255, 255]),
                transition,
            ],
            color_space: ColorSpace::default(),
            canvas: [0, 0, 0, 255],
        };
        let pool = concat_media::ReaderPool::new(1024 * 1024, 2);
        let sources = preview_sources(&pool, &request).unwrap();
        let pixel = sources
            .composite(&mut concat_render::CpuCompositor)
            .pixel(8, 8)
            .unwrap();
        assert!(
            (120..=135).contains(&pixel[0]) && (120..=135).contains(&pixel[2]),
            "{pixel:?}"
        );
        assert_eq!(pixel[3], 255);
        let span = sources
            .transitions
            .iter()
            .find(|span| span.layered)
            .unwrap();
        let (from, to) = transition_stages(sources.plan(), span);
        assert_eq!(from.layers.len(), 1);
        assert_eq!(to.layers.len(), 1);
    }
    #[test]
    fn the_cpu_transition_does_not_darken_transparent_edges() {
        let red = Frame::from_rgba(1, 1, vec![255, 0, 0, 255]).unwrap();
        let clear = Frame::transparent(1, 1);
        assert_eq!(mix(&red, &clear, 0.5).pixel(0, 0), Some([255, 0, 0, 128]));
        let rgba: Vec<u8> = [65535u16, 0, 0, 65535]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect();
        let red = Frame::from_rgba64(1, 1, rgba, Signal::SdrWide).unwrap();
        let clear = Frame::from_rgba64(1, 1, vec![0; 8], Signal::SdrWide).unwrap();
        let mixed = mix(&red, &clear, 0.5);
        let channels: Vec<_> = mixed
            .pixels()
            .chunks_exact(2)
            .map(|x| u16::from_le_bytes([x[0], x[1]]))
            .collect();
        assert_eq!(channels, vec![65535, 0, 0, 32768]);
    }
}
