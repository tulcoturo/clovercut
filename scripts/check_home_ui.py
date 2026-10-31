#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Exercise the actual home screen with Slint's simulated keyboard/accessibility backend.

No native window, GPU, audio, personal profile or copied UI implementation.
This complements full desktop CI; it does not qualify physical accessibility.
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
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    workspace = root / "src"
    fixtures = root / "scripts/fixtures/home-ui"
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
            else:
                manifest += f'{name} = {pinned}\n'
        (scratch / "Cargo.toml").write_text(manifest)
        (scratch / "Cargo.lock").write_bytes(lock)
        for name in ("main.rs", "build.rs"):
            (scratch / name).write_bytes((fixtures / name).read_bytes())
        source = json.dumps(str(workspace / "crates/concat/ui/start.slint"))[1:-1]
        (scratch / "probe.slint").write_text((fixtures / "probe.slint").read_text().replace("__START_SCREEN__", source))
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
