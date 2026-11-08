#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Exercise actual home, timeline, caption or catalogue controls with Slint's testing backend.

No native window, GPU, audio, personal profile or copied UI implementation.
The home suite paints with the software renderer and bundled fonts; other
suites use the mock renderer. This complements full desktop CI and does not
qualify physical accessibility or native OS dialogs.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--suite", choices=("home", "timeline", "captions", "effects", "publish", "scroll"), default="home")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    workspace = root / "src"
    fixtures = root / "scripts/fixtures" / f"{args.suite}-ui"
    lock = (workspace / "Cargo.lock").read_bytes()
    packages = tomllib.loads(lock.decode())["package"]
    version = tomllib.loads((workspace / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    with tempfile.TemporaryDirectory(prefix="clovercut-home-ui-") as directory:
        scratch = Path(directory)
        manifest = (f'[package]\nname = "clovercut-home-ui-check"\nversion = {json.dumps(version)}\n'
                    'edition = "2024"\n[[bin]]\nname = "home-ui-check"\npath = "main.rs"\n'
                    '[dependencies]\n')
        for name in ("slint", "i-slint-backend-testing", "slint-build"):
            versions = {package["version"] for package in packages if package["name"] == name}
            if len(versions) != 1:
                parser.error(f"ambiguous locked versions for {name}")
            pinned = json.dumps("=" + versions.pop())
            if name == "slint":
                manifest += f'{name} = {{version = {pinned}, default-features = false, features = ["std", "compat-1-2"]}}\n'
            elif name == "slint-build":
                manifest += f'[build-dependencies]\n{name} = {pinned}\n'
            elif args.suite == "home":
                manifest += f'{name} = {{version = {pinned}, features = ["renderer-software"]}}\n'
            else:
                manifest += f'{name} = {pinned}\n'
        (scratch / "Cargo.toml").write_text(manifest)
        (scratch / "Cargo.lock").write_bytes(lock)
        for name in ("main.rs", "build.rs"):
            (scratch / name).write_bytes((fixtures / name).read_bytes())
        if args.suite == "publish":
            studio = (workspace / "crates/concat/src/studio.rs").read_text()
            first = studio.index("pub fn sync<T:")
            last = studio.index("/// The window's state.", first)
            (scratch / "publish_helpers.rs").write_text(studio[first:last])
        source = json.dumps(str(workspace / "crates/concat/ui/start.slint"))[1:-1]
        theme = json.dumps(str(workspace / "crates/concat/ui/theme/theme.slint"))[1:-1]
        scrollbar = json.dumps(str(workspace / "crates/concat/ui/primitives/scrollbar.slint"))[1:-1]
        lanes = json.dumps(str(workspace / "crates/concat/ui/timeline/lanes.slint"))[1:-1]
        model = json.dumps(str(workspace / "crates/concat/ui/timeline/model.slint"))[1:-1]
        captions_dialog = json.dumps(str(workspace / "crates/concat/ui/dialogs/captions.slint"))[1:-1]
        captions_panel = json.dumps(str(workspace / "crates/concat/ui/workspace/captions-panel.slint"))[1:-1]
        media_pane = json.dumps(str(workspace / "crates/concat/ui/workspace/media-pane.slint"))[1:-1]
        transition_preview = json.dumps(str(workspace / "crates/concat/ui/workspace/transition-preview.slint"))[1:-1]
        primitives = json.dumps(str(workspace / "crates/concat/ui/primitives/primitives.slint"))[1:-1]
        probe = (fixtures / "probe.slint").read_text().replace(
            "__FONTS__", json.dumps(str(workspace / "crates/concat-text/fonts"))[1:-1])
        for marker, relative in {
                "__INSPECTOR__": "inspector/clip-inspector.slint",
                "__EXPORT__": "dialogs/export.slint",
                "__SETTINGS__": "dialogs/settings.slint",
                "__RELINK__": "dialogs/relink.slint",
                "__START__": "start.slint",
                "__CAPTIONS__": "dialogs/captions.slint",
        }.items():
            probe = probe.replace(marker, json.dumps(str(workspace / "crates/concat/ui" / relative))[1:-1])

        (scratch / "probe.slint").write_text(
            probe.replace("__MEDIA_PANE__", media_pane).replace("__TRANSITION_PREVIEW__", transition_preview).replace("__PRIMITIVES__", primitives).replace("__START_SCREEN__", source).replace("__SCROLLBAR__", scrollbar).replace("__THEME__", theme)
                .replace("__LANES__", lanes).replace("__TIMELINE_MODEL__", model)
                .replace("__CAPTIONS_DIALOG__", captions_dialog).replace("__CAPTIONS_PANEL__", captions_panel)
        )
        env = os.environ.copy()
        env.setdefault("CARGO_TARGET_DIR", str(workspace / "target"))
        env.setdefault("CARGO_BUILD_JOBS", "2")
        env.setdefault("CARGO_PROFILE_DEV_DEBUG", "0")
        env.setdefault("CARGO_PROFILE_DEV_CODEGEN_UNITS", "16")
        for command, tail in (("clippy", ["--all-targets", "--", "-D", "warnings"]), ("run", [])):
            result = subprocess.run(
                ["cargo", command, "--manifest-path", str(scratch / "Cargo.toml"),
                 *(["--offline"] if args.offline else []), *tail],
                cwd=workspace, env=env, check=False,
            )
            if result.returncode:
                return result.returncode
    if (workspace / "Cargo.lock").read_bytes() != lock:
        raise RuntimeError("workspace lockfile changed during isolated UI checks")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
