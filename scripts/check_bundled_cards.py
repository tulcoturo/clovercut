#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Clovercut contributors
"""Check every visual package's bundled JPEG before compiling the native UI.

This checks presence and JPEG frame dimensions; the native Studio regression
also fully decodes each image with the application's image loader.
"""
from pathlib import Path
import struct
import tomllib


def jpeg_size(path):
    data = path.read_bytes()
    if data[:2] != b'\xff\xd8':
        raise ValueError('not a JPEG')
    at = 2
    while at < len(data):
        if data[at] != 0xff:
            raise ValueError('invalid JPEG marker')
        while at < len(data) and data[at] == 0xff:
            at += 1
        marker = data[at]
        at += 1
        if marker in (0xd9, 0xda):
            break
        if marker == 0x01 or 0xd0 <= marker <= 0xd7:
            continue
        length, = struct.unpack_from('>H', data, at)
        if length < 2 or at + length > len(data):
            raise ValueError('truncated JPEG segment')
        if marker in (0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7,
                      0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf):
            height, width = struct.unpack_from('>HH', data, at + 3)
            return width, height
        at += length
    raise ValueError('JPEG has no frame header')


def main():
    root = Path(__file__).resolve().parent.parent / 'src/crates'
    errors = []
    count = 0
    for manifest in sorted((root / 'concat-effects/packages').glob('*/effect.toml')):
        effect = tomllib.loads(manifest.read_text())['effect']
        package = effect['id']
        if effect['kind'] == 'audio' or package == 'concat.adjust':
            continue
        count += 1
        try:
            size = jpeg_size(root / 'concat/cards' / f'{package}.jpg')
            if size != (480, 270):
                errors.append(f'{package}: {size}, expected (480, 270)')
        except (OSError, ValueError, IndexError, struct.error) as error:
            errors.append(f'{package}: {error}')
    if errors:
        print('\n'.join(errors))
        return 1
    print(f'{count} visual packages have bundled 480x270 JPEG cards')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
