# Carried voices and release candidate 0.2.6

Adapts cb895fe8 without reassigning Clovercut's saved Bria/clone IDs. Four
public-domain LibriVox references use IDs 2100–2103 and are available without
Pocket; Elizabeth is the default for new Chatterbox selections. Existing Kokoro,
Pocket and legacy Chatterbox references remain available. Kokoro no longer
incorrectly requires Pocket's recording directory. The references decode to ten
seconds each; the exact-source fixture verifies all four and repairs a corrupt
cache of the same length. Strict standalone Clippy passed.

The pinned Spleeter models were downloaded into scratch storage and verified
against the manifest's sizes/SHA256. Actual music-model inference passed the
carried-voice reconstruction test, including sample counts and summed stems.
This closes the earlier audit's pending music inference check.

CI 43472082184 compiled and linted the native window and Android shell, then
passed 1656/1657 tests. The DFN3 inference test aborted with invalid free in the
full speech-linked binary, while the standalone engine passed. The candidate
hides Linux static-archive symbols from dynamic libraries to isolate sherpa's
static ONNX/protobuf symbols from the separately loaded runtime. This candidate
fix still requires the gated release's native inference test; it is not marked
as confirmed merely because the standalone engine passed.

0.2.6 is a new tag/version because the failed 0.2.5 preparation already has a
tag. The release workflow must pass all existing checks before publishing.
Full parity against all upstream commits has not been claimed or established.
