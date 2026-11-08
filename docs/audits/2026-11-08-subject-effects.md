# Subject effects

Two source-clip effects, `concat.subject-neon` and `concat.subject-electric`, extend the existing person segmentation service. They keep the original background and add animated light around the detected silhouette. This is silhouette detection, without face landmarks or accessory tracking.

Enabled effects request person analysis when the clip has no cutout. Existing cutouts retain their chosen subject and edited mask. Bypass/removal cancels obsolete analysis; callbacks queue the next wanted source. Analysis retains the existing 10 Hz source-time grid; shader animation follows continuous clip time. This can limit tracking quality on fast motion and needs footage review.

Export and preview attach cached, bounded (512 maximum side) eight-bit coverage masks without converting deep source colours to eight bits. Mask-only jobs cannot cut/highlight the frame or make opaque content transparent. Unanalysed/empty masks leave the source unchanged. Mask coordinates compose with decoder and compositor crop, flips and stabilization warp.

Subject decoration runs before the ordinary picture chain so later distortions move the picture and aura together. Stable ordering retains relative order within both stages, with inspector reorder controls respecting the boundary. Project commands reject subject effects on layer/title/shape clips before mutation. No cutout field is added to documents merely by applying these effects.

The GPU retains four bind groups. New mask bindings share the reveal group; the small mask and affine transform participate in its bounded cache key. Subject shader helpers are injected only for these two packages, preserving existing shader source/cache keys.

The dedicated Subject category has actual shader-driven animated person thumbnails. Their source is documented in [asset provenance](../assets/subject-card.md). No model inference or download occurs on hover.

Validation includes package probes, project stage ordering and atomic rejection, mask geometry/alpha/movement GPU tests, analysis request selection and bypass, mask-only export behavior, and real thumbnail animation with bundled person inference. Native and visual results must be reviewed before integration.
