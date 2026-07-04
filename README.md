<div align="center">

# V.I.B.E

### Clap twice. Your workspace appears.

A macOS menu bar app that turns a double clap into your work setup:
your apps launch, your tabs open, and every window lands in the
screen layout you designed.

[![Release](https://img.shields.io/github/v/release/team-poem/vibe?color=e8531f)](https://github.com/team-poem/vibe/releases/latest)
![macOS](https://img.shields.io/badge/platform-macOS-black?logo=apple)
![Tauri](https://img.shields.io/badge/Tauri_2-Rust-24C8DB?logo=tauri&logoColor=white)
![License](https://img.shields.io/badge/license-MIT-green)

<!-- demo gif: double clap → workspace assembling -->

</div>

## Why

Starting work means the same ritual every time: open the editor, open the
tabs, drag windows around, start the music. V.I.B.E compresses that ritual
into one gesture you always have with you.

## Features

- **Double-clap trigger.** A rule-based audio engine listens on-device and
  tells your claps apart from typing, speech, and music.
- **Routines.** Name a setup, stack app and URL actions, switch the active
  one from the menu bar.
- **Screen layout.** Drag actions onto a monitor mockup, split into halves,
  thirds, or quadrants. Windows snap into place as they open.
- **Fast.** Clap to first action in about 150 ms.
- **Private.** No account, no server, no stored audio. One local JSON file.

## Install

Grab the latest `.dmg` from the
[**latest release**](https://github.com/team-poem/vibe/releases/latest)
and drag **V.I.B.E** into Applications.

- Apple Silicon only, for now.
- Builds are not notarized yet. If macOS blocks the first launch:
  System Settings → Privacy & Security → **Open Anyway**, or
  `xattr -cr /Applications/V.I.B.E.app`.

First run: pick a language, create a routine, set it active, allow the
microphone (plus Accessibility if you place windows). Clap twice.

## Under the hood

```
mic (cpal) → streaming clap detector → double-clap matcher → action runner → window placer (AX API)
```

- **Detection** is rule-based DSP, not ML: adaptive noise floor (EMA),
  FFT spectral flatness, decay gating, refractory windows. Tuned for zero
  false positives on typing/speech/music test recordings.
- **Matching** pairs claps 150–600 ms apart with similar peak and spectrum.
- **Execution** is two-phase: all actions launch first, then windows are
  awaited and placed, so one cold app never delays the rest. New browser
  windows are identified by diffing the AX window list.
- **Threading**: capture, detection, and action execution each run on their
  own thread with channels in between; the UI can never block a trigger.
- **Stack**: Tauri 2, Rust (`cpal`, `rustfft`, AXUIElement FFI), React + TS.

## Build from source

Requires Rust (stable), Node.js, pnpm.

```bash
pnpm install
pnpm tauri dev      # development
pnpm tauri build    # .app + .dmg
```

## Docs

- [`spec/prd.md`](spec/prd.md) — product requirements
- [`spec/history.md`](spec/history.md) — development journal, PoC to present
- `poc/*` branches — standalone proofs of concept, kept unmerged as reference

## License

MIT © team-poem
