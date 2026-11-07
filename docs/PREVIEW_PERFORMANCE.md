# Preview performance on Windows

A laptop without a dedicated GPU may still have a usable integrated GPU. First
identify the adapter Clovercut actually opened: **Settings > About > System
information** provides a copy button and **Show the log**. Include the Clovercut
version, CPU, RAM, GPU/driver, rendering mode and the media's codec, resolution
and frame rate when reporting a problem.

**Microsoft Basic Render Driver** or **software rasteriser** means the preview
is being rendered on the CPU through WARP. That can happen when software mode is
selected or when the hardware cannot open the supported graphics backend. It is
different from using an Intel/AMD integrated GPU. Record whether **Draw on the
GPU** was enabled and whether the application used safe start; those facts help
distinguish a fallback from a decoding or rendering bottleneck.

Try one short clip without effects, then compare Full, Half and Quarter preview
quality on the same section. Lowering quality reduces the rendered frame size;
it does not replace the source video's decoding workload with a smaller file.
For high-resolution media, **Settings > General > Proxies** can create smaller
copies in the background. Once a proxy is ready, playback at Half/Quarter can
use it; Full deliberately uses the original. Proxies remain opt-in.

After reproducing and stopping playback, the log can include a `playback:` line
with frames shown, repeated/skipped frames and the worst frame-request wait.
The `at ... fps` value is the timeline's requested rate, **not a measured achieved
frame rate**. Device-lost messages and the adapter line are also useful. Include
whether the problem is continuous playback, dragging the playhead, or both.

The 0.2.3 work adds approximate scrubbing/exact rest refinement, lower publication
churn and lost-device recovery guards. Software-Windows shader checks validate
pipeline correctness; they cannot establish playback performance on a particular
laptop. [Discussion #28](https://github.com/tulcoturo/clovercut/discussions/28)
remains a hardware-specific report requiring measurements on the affected laptop.
