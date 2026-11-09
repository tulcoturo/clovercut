<p align="center">
  <img src="assets/typography.svg" alt="Clovercut" width="460" />
</p>

<p align="center"><strong>Local editing. Creative effects. No subscription or export watermark.</strong></p>

<p align="center">
  <a href="https://tulcoturo.github.io/clovercut/">Website</a> ·
  <a href="https://github.com/tulcoturo/clovercut/releases">Download</a> ·
  <a href="docs/README.md">Documentation</a> ·
  <a href="https://github.com/tulcoturo/clovercut/issues">Report an issue</a>
</p>

Clovercut is a free, open-source video editor for **Linux, Windows and macOS**.
It builds on [Concat](https://github.com/jub0t/Concat), with its own desktop
workflow: a fixed main video track, dedicated visual and audio lanes, workspace
presets and searchable content libraries. Editing and AI processing run locally,
with no Clovercut account, subscription or export watermark.

![Clovercut desktop workspace with creative libraries, preview, inspector and colour-coded timeline](assets/screenshots/editor-workspace.png)

## Why choose Clovercut?

Clovercut brings a familiar desktop editing workflow, local AI and playful visual
effects together in one free, open-source application. If you are comparing video
editors, these are the priorities Clovercut is built around:

| What matters to you | What Clovercut offers |
|---|---|
| A straightforward free editor | No Clovercut account, subscription or export watermark. |
| Control over your footage | Editing, rendering and AI inference run on your computer. Optional online libraries have their own setup and terms. |
| Linux alongside Windows and macOS | Desktop packages for x86_64 and ARM64, including AppImage, deb and rpm on Linux. |
| Creative effects with local analysis | Neon and electricity around a person, plus sunglasses and dog accessories that follow detected facial landmarks. |
| A familiar editing flow | Typed timeline lanes, workspace presets and configurable shortcuts, including profiles for other editors. |
| An open project you can inspect and extend | AGPL-3.0 source, keyframeable effects and filters, GL Transitions and an MCP automation interface. |

Clovercut focuses on desktop editing, creator tools and local processing. The facial accessories are 2D effects;
tracking quality depends on the footage and hardware. See the
[release notes](docs/releases/0.2.4.md) for the current scope and qualification.

## What Clovercut adds

Clovercut develops its own editing experience on top of Concat's engine.
Its main additions and workflow differences are:

- **An organised timeline.** A fixed main video track anchors the edit, with
  overlays, titles, captions and stickers above it and audio below. Track types
  have distinct colours; adjustment layers stay within the visual zone. Cuts
  follow the selection or active track, and optional linkage respects track locks.
- **Workspace and shortcut presets.** Layouts for vertical video, audio editing,
  media browsing and detailed inspection. Shortcut profiles for CapCut,
  Premiere Pro, Final Cut Pro and DaVinci Resolve, with custom bindings.
- **Integrated content sources.** Browse OpenGameArt, Freesound and the YouTube
  Audio Library for music and sound effects, and GIPHY for animated stickers.
  Preview assets and add them to the timeline from the editor.
- **Personal creative libraries.** Keep favourites, recently used and most-used
  effects, filters, transitions and stickers across projects.
- **Person and face effects.** Subject Neon and Subject Electric decorate the
  detected silhouette while keeping the background. Face Sunglasses and Dog Face
  follow facial landmarks, with a bundled local detector and reusable analysis.
  Animated effect/filter thumbnails show the actual shaders; fallback cards ship
  with the application.
- **An expanded transition library.** Browse 123 supported GL Transitions,
  download them on demand and preview them by hovering over their cards.
  Installed transitions remain available offline.

Local Whisper captions, text-to-speech, GPU compositing, keyframes and export
build on the capabilities inherited from Concat. Clovercut selectively adapts
upstream improvements to its own timeline and project behaviour; the
[adaptation record](docs/UPSTREAM.md) documents those decisions.

## Download and install

[**Download Clovercut 0.2.4**](https://github.com/tulcoturo/clovercut/releases/tag/v0.2.4)
— [Release notes](docs/releases/0.2.4.md)

| Platform | Packages | Architectures |
|---|---|---|
| Linux | AppImage, `.deb`, `.rpm` | x86_64, ARM64 |
| Windows | Installer or portable ZIP | x86_64, ARM64 |
| macOS | DMG | Intel, Apple Silicon |

Linux installer:

```sh
curl -fsSL https://raw.githubusercontent.com/tulcoturo/clovercut/main/scripts/install.sh | sh
```

See [Linux installation](docs/INSTALLATION.md) for package selection, manual
installation and graphics fallback, and [macOS installation](docs/MACOS_INSTALLATION.md)
for first-launch and Gatekeeper guidance. Desktop releases are the priority;
mobile releases are out of scope for now. Changes on `main` may be newer than the
published packages.

## Local processing, optional online libraries

Editing, rendering and AI inference run on your computer. Some AI features require an initial model download and can then be used offline.
The facial detector is bundled and needs no model download. Online content libraries are
optional and disabled by default.

Freesound requires an API key. The YouTube Audio Library uses a bundled local
connector and a dedicated browser sign-in. Online assets retain their providers'
licences and attribution requirements. See [content library setup](docs/AUDIO_LIBRARY.md).

## Documentation and development

- [Editing guide](docs/EDITING.md) and [keyboard shortcuts](docs/KEYBOARD_SHORTCUTS.md)
- [Content libraries](docs/AUDIO_LIBRARY.md), [personal lists](docs/LIBRARY_ORGANIZATION.md) and [GL Transitions](docs/GL_TRANSITIONS.md)
- [Build from source](docs/RELEASING.md#local-development) and [native dependencies](src/README.md#build-and-run)
- [Automation with MCP](docs/MCP.md)
- [Contributing](CONTRIBUTING.md) and [translations](TRANSLATING.md)

## Licence and attribution

Clovercut is an independent project based on Concat by Jareer and the Concat
contributors, whose engine and contributions remain credited. Distributed under
[AGPL-3.0-or-later](LICENSE). See [third-party notices](THIRD_PARTY_NOTICES.md)
and [trademark information](TRADEMARK.md).
