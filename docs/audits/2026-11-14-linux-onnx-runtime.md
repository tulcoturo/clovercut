# Linux ONNX runtime selection

The diagnostic run [43585706324](https://github.com/tulcoturo/clovercut/actions/runs/43585706324)
captured the voice-isolation abort inside the executable's static ONNX runtime:
`OrtApis::CreateEnvWithCustomLogger` → environment initialization → device
discovery → `std::regex` construction → `free(): invalid pointer`.
The failure precedes inference. Hiding archive exports did not prevent the
executable's direct `OrtGetApiBase` reference selecting the static implementation.

Linux inference now enables ort's dynamic loader and initializes it from the
bundled `lib/libonnxruntime.so`, an explicit `ORT_DYLIB_PATH`, or the developer/CI
loader path. The API comes from that library's handle rather than the executable.
Linux packaging also retains archive-symbol hiding when overriding Cargo's
repository linker flags with its bundle rpath.

The regression checks `/proc/self/maps` to verify that the actual API table lives
in `libonnxruntime.so`. A small fixture reusing the production isolation and
runtime sources links the real sherpa static archives alongside ONNX 1.28 and
uses the checksum-pinned CI FFmpeg. The real speech separation and API-origin
checks are run locally; the complete application remains gated by release CI.
