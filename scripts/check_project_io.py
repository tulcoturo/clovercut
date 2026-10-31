#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Check real project/template/preferences storage without native media/UI.

The temporary crate reuses the source, dependency versions and lockfile from this
checkout. It never copies implementation logic or changes the workspace lockfile.
This narrow check complements the full host/release CI.
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
    parser.add_argument("--offline", action="store_true", help="use already cached dependencies")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    workspace = root / "src"
    lock = (workspace / "Cargo.lock").read_bytes()
    packages = tomllib.loads(lock.decode())["package"]
    version = tomllib.loads((workspace / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    with tempfile.TemporaryDirectory(prefix="clovercut-project-io-") as directory:
        scratch = Path(directory)
        manifest = (
            '[package]\nname = "clovercut-project-io-check"\n'
            f'version = {json.dumps(version)}\nedition = "2024"\n'
            '[lib]\npath = "lib.rs"\n'
            '[dependencies]\n'
        )
        for name in ("serde", "serde_json", "log", "tempfile"):
            versions = {package["version"] for package in packages if package["name"] == name}
            if len(versions) != 1:
                parser.error(f"ambiguous locked versions for {name}; update the isolated check")
            pinned = json.dumps("=" + versions.pop())
            manifest += (f'{name} = {{ version = {pinned}, features = ["derive"] }}\n'
                         if name == "serde" else f'{name} = {pinned}\n')
        manifest += 'concat-project = { path = ' + json.dumps(str(workspace / "crates/concat-project")) + ' }\n'
        manifest += 'concat-effects = { path = ' + json.dumps(str(workspace / "crates/concat-effects")) + ' }\n'
        (scratch / "lib.rs").write_text(
            'extern crate self as concat_host;\n'
            '#[path = ' + json.dumps(str(workspace / "crates/concat-host/src/dirs.rs")) + '] pub mod directories;\n'
            'pub use directories::AppDirs;\n'
            '#[path = ' + json.dumps(str(workspace / "crates/concat-host/src/library.rs")) + '] pub mod library;\n'
            '#[path = ' + json.dumps(str(workspace / "crates/concat/src/prefs.rs")) + '] pub mod prefs;\n'
            '#[path = ' + json.dumps(str(workspace / "crates/concat/src/project_browser.rs")) + '] pub mod project_browser;\n'
            '#[path = ' + json.dumps(str(workspace / "crates/concat-host/src/projects.rs")) + '] pub mod projects;\n'
            '#[path = ' + json.dumps(str(workspace / "crates/concat-host/src/templates.rs")) + '] pub mod templates;\n'
            # Poster extraction requires native media libraries and is covered by full CI.
            'pub mod media { pub fn poster_frame(_: &str) -> Result<Vec<u8>, String> { Err("native poster extraction unavailable in storage-only check".into()) } }\n'
        )
        (scratch / "Cargo.toml").write_text(manifest)
        (scratch / "Cargo.lock").write_bytes(lock)
        env = os.environ.copy()
        env.setdefault("CARGO_TARGET_DIR", str(workspace / "target"))
        env.setdefault("CARGO_BUILD_JOBS", "2")
        for command, options in (
                ("clippy", ["--all-targets", "--", "-D", "warnings"]),
                ("test", ["--lib"])):
            process = subprocess.run(
                ["cargo", command, "--manifest-path", str(scratch / "Cargo.toml"),
                 *(["--offline"] if args.offline else []), *options],
                cwd=workspace, env=env, check=False,
            )
            if process.returncode:
                return process.returncode
    if (workspace / "Cargo.lock").read_bytes() != lock:
        raise RuntimeError("workspace lockfile changed during isolated project checks")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
