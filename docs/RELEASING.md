# Building and releasing Clovercut

Clovercut's primary release targets are desktop: Linux x86_64/aarch64 AppImages,
Windows x86_64/ARM64 installers and portable ZIPs, and macOS Intel/Apple Silicon
DMGs. The matrix is defined in `scripts/ci/desktop-targets.json`; native dependency
downloads are checksum-pinned in `scripts/ci/deps.lock`. Mobile builds follow a
separate schedule and are disabled in the desktop release workflow.

## Local development

Run Cargo from `src/` to use the pinned toolchain. Install the native dependencies
described in the [source overview](../src/README.md), then use the existing profile:

```sh
cargo build --profile quick -p concat
./target/quick/clovercut --version
./target/quick/clovercut --smoke
```

Load any machine-local native-library environment without committing credentials
or absolute machine paths. Keep one heavy graphical Rust target at a time. On a
16 GiB machine, the generated UI can require temporary disk swap beyond zram;
provision headroom before compiling and remove temporary swap after validation.
Do not change build profiles or disable OOM protections to work around pressure.

A validated run on the 16 GiB Linux development machine used one Cargo job,
`--offline --locked --profile quick -p concat --bin clovercut`, two CPU cores,
`Nice=10`, an 8 GiB RAM ceiling and a 14 GiB swap allowance. A 16 GiB temporary
Btrfs swap file at priority 200 took precedence over the existing priority-100
zram. The guard stopped the build if `MemAvailable` fell below 2.5 GiB; only use
that RAM ceiling when other applications leave that headroom. Provision the disk
swap before starting, and arrange automatic `swapoff` and removal after validation.
Never remove a swap file while it is active.

That complete UI rebuild peaked at 8 GiB RAM and 10.5 GiB swap and took about
44 minutes under those limits. It finished with a successful fresh-profile
`--smoke` run, including Vulkan adapter creation and Whisper initialization.
Incremental/no-change builds can have very different resource and time costs.

A local development executable is not a portable release package.

### macOS 12 media runtime

Desktop packaging rebuilds FFmpeg and the required codec/audio dependencies from
the checksum-pinned `macos-source-*` entries in `scripts/ci/deps.lock`. Homebrew
provides build tools; its runtime bottles are not a Monterey-compatible SDK.
Setting `MACOSX_DEPLOYMENT_TARGET=12.0` on Rust does not lower a prebuilt library's
minimum or remove its newer API imports. The published 0.1.9 bundle illustrates
that failure; see [issue #2](https://github.com/tulcoturo/clovercut/issues/2).

On a native Intel or Apple Silicon Mac with Xcode and the workflow's build tools,
choose a new empty prefix and run, for example:

```sh
MACOSX_DEPLOYMENT_TARGET=12.0 scripts/build-macos-media.sh aarch64-apple-darwin /tmp/clovercut-media-arm64
python3 scripts/ci/smoke-macos-media.py /tmp/clovercut-media-arm64
```

The recipe uses two compilation jobs by default and preserves x264, multilib
x265, VP8/VP9, Opus, AOM/SVT-AV1, dav1d and Rubber Band with libsamplerate.
VideoToolbox decode/encode stays enabled; the unused `transpose_vt` filter is
excluded because its rotation-session API requires macOS 13. Availability
warnings for newer SDK APIs are errors. The cache key includes architecture,
recipe and pinned source rows; only a complete, codec-tested prefix is cached.

Codec smoke checks encode/decode H.264, 8/10-bit HEVC and AV1, preserve VP9/ProRes
alpha, and exercise AAC, Opus and Rubber Band duration. Packaging then enforces
every eagerly loaded Mach-O library's declared minimum. Intel ONNX Runtime
retains its explicit first-use exception and runtime OS guard; it is not linked
at launch. Source URLs, build configuration and library licences accompany the
bundle. Header/modern-runner checks do not establish execution on an actual
Monterey machine; retain that distinction in qualification records.

### Optional graphical walkthrough

The Linux screenshot driver is development tooling, excluded from ordinary and
release builds. In an isolated Hyprland session with `grim` and FFmpeg available:

```sh
CLOVERCUT_UI_WALKTHROUGH=1 cargo run --profile quick -p concat --bin clovercut --features ui-walkthrough -j1
```

Use a fresh XDG profile and a dedicated capture directory (`CLOVERCUT_SCREENSHOT_DIR`).
Missing fixture generation or screenshots fail the run. The driver still uses
timed steps; captures require visual review and do not replace interactive focus,
accessibility or cross-platform checks.

This is a screenshot fixture, not an end-to-end qualification: import waits for
all fixture paths with a 60-second deadline, missing-media and save-warning dialogs are staged,
and the export steps configure a dialog without completing an export. Do not
use its completion or screenshots as evidence of successful recovery or export.
Other layout captures remain timed. The separate state-driven editor journey in
[TESTING.md](TESTING.md#opt-in-runtime-performance-report) exercises actual
asynchronous recovery and export; neither driver qualifies mouse/keyboard input.

## Prepare a release

1. Set the workspace version in `src/Cargo.toml` and the workspace lockfile entries.
2. Write `docs/releases/<version>.md`. Include only features present in that source.
3. Run formatting, strict Clippy, tests, locale/model inventories and applicable
   native startup/editing/export checks.
4. Commit and push the intended source. The manual **Build Clovercut packages**
   workflow accepts an exact `ref`, a desktop target and `include_mobile=false`.
   It produces artifacts without publishing. A successful build of an older tag
   does not include newer branch changes.
5. Push the matching version tag when ready to publish. **Clovercut release**
   resolves one source commit and runs CI plus all desktop package jobs. It
   publishes automatically only after both CI and packaging succeed.

The release workflow also supports manual `tag` and `ref` inputs. Use a consistent
source/tag identity; do not silently replace a published version. Workflow files
are authoritative for triggers, runners, build settings and checks.

## Package validation

Linux jobs validate staged binaries, portable AppImage startup and x86_64 CPU
compatibility. Windows jobs validate native DLL loading, clean-PATH startup and
architecture/CPU compatibility. macOS jobs check the app bundle and native startup.
FFmpeg packages must include the required Rubber Band filters. Windows dynamic
GGML builds retain their backend feature probes and checksum-pinned dependencies.

Linux builds also install their `.deb` in a clean Debian 12 container and their
`.rpm` in Fedora 43, on each native architecture. The checks resolve dependencies,
launch the installed executable and graphical smoke test, reinstall the package,
then remove it while preserving an application-data marker. This qualifies the
package lifecycle; it does not simulate a binary/schema upgrade from an older
release. All package-manager mutations happen inside disposable containers.

To run those lifecycle checks against an already-published stable version without
rebuilding the editor, dispatch **Linux packaging checks** with `release_tag`
(for example `v0.1.9`) and the full source SHA in `ref`. Release downloads are
checksum-verified before installation.

Every desktop release after 0.1.8 requires these twelve nonempty packages:

- `Clovercut-<version>-x86_64.AppImage`
- `Clovercut-<version>-aarch64.AppImage`
- `Clovercut-<version>-x86_64.deb`
- `Clovercut-<version>-aarch64.deb`
- `Clovercut-<version>-x86_64.rpm`
- `Clovercut-<version>-aarch64.rpm`
- `Clovercut-<version>-windows-x86_64-setup.exe`
- `Clovercut-<version>-windows-x86_64-portable.zip`
- `Clovercut-<version>-windows-aarch64-setup.exe`
- `Clovercut-<version>-windows-aarch64-portable.zip`
- `Clovercut-<version>-macos-arm64.dmg`
- `Clovercut-<version>-macos-x86_64.dmg`

The workflow adds `manifest.json` for the updater and `SHA256SUMS`. Verify the
published assets and source identity before updating the README/site download
links. Ordinary main-branch pushes do not publish a release or start package builds.

`scripts/build-appimage.sh STAGE ARCH OUTPUT.AppImage` packages a prepared Linux
stage. It needs the executable, runtime libraries, desktop entry, icon, licence
and third-party notices. Use the workflow's shipping profile and dependency setup.

Application state belongs to `io.github.tulcoturo.clovercut`. Project folders
retain the inherited format, and updater metadata points to Clovercut releases.
