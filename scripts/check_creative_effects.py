#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Clovercut contributors
"""Render real creative-effect shaders without native media/UI dependencies.

Requires a GPU or a Vulkan software adapter. Optional input is 480x270 raw RGBA;
otherwise the fixture draws a synthetic chart. PPM previews are optional.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--offline', action='store_true')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--input-rgba', type=Path)
    parser.add_argument('--animate', action='store_true', help='write 24 actual shader frames per effect')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    workspace = root / 'src'
    original = (workspace / 'Cargo.lock').read_bytes()
    with tempfile.TemporaryDirectory(prefix='clovercut-creative-effects-') as directory:
        scratch = Path(directory)
        manifest = '[package]\nname = "clovercut-creative-effects-check"\nversion = "0.0.0"\nedition = "2024"\n[[bin]]\nname = "creative-effects-check"\npath = "main.rs"\n[dependencies]\n'
        for name in ('concat-core', 'concat-render', 'concat-effects'):
            manifest += name + ' = { path = ' + json.dumps(str(workspace / 'crates' / name)) + ' }\n'
        (scratch / 'Cargo.toml').write_text(manifest)
        (scratch / 'Cargo.lock').write_bytes(original)
        (scratch / 'main.rs').write_bytes((root / 'scripts/fixtures/creative-effects/main.rs').read_bytes())
        env = os.environ.copy()
        env.setdefault('CARGO_TARGET_DIR', str(workspace / 'target'))
        env.setdefault('CARGO_BUILD_JOBS', '1')
        env.setdefault('CARGO_PROFILE_DEV_DEBUG', '0')
        if args.animate:
            env['CLOVERCUT_EFFECTS_QA_ANIMATE'] = '1'
        if args.output:
            env['CLOVERCUT_EFFECTS_QA_OUTPUT'] = str(args.output.resolve())
        if args.input_rgba:
            env['CLOVERCUT_EFFECTS_QA_RGBA'] = str(args.input_rgba.resolve())
        for command, tail in [('clippy', ['--all-targets', '--', '-D', 'warnings']), ('run', [])]:
            result = subprocess.run(['cargo', command, '--manifest-path', str(scratch / 'Cargo.toml'), *(['--offline'] if args.offline else []), *tail], cwd=workspace, env=env, check=False)
            if result.returncode:
                return result.returncode
    if (workspace / 'Cargo.lock').read_bytes() != original:
        raise RuntimeError('workspace lockfile changed')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
