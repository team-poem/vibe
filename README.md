<div align="center">

# V.I.B.E

Clap twice. Your workspace appears.

![macOS](https://img.shields.io/badge/platform-macOS-black?logo=apple)
![Tauri](https://img.shields.io/badge/Tauri_2-Rust-24C8DB?logo=tauri&logoColor=white)
![License](https://img.shields.io/badge/license-MIT-green)

</div>

V.I.B.E is a macOS menu bar app that detects a double clap and runs your
routine: launch apps, open URLs, and snap each window into a screen layout
you set on a monitor mockup.

Everything runs on-device. No account, no server, no stored audio.

## Install

Download the latest `.dmg` from [Releases](https://github.com/team-poem/vibe/releases),
open it, and drag **V.I.B.E** into Applications.

- Apple Silicon only.
- The build is not notarized yet. If macOS blocks the first launch:
  System Settings → Privacy & Security → **Open Anyway**, or run
  `xattr -cr /Applications/V.I.B.E.app`.

## Usage

1. Click **V** in the menu bar → Show settings.
2. Create a routine: add app / URL actions, pick a split (2·3·4), click a
   region on the monitor to place each action.
3. Set the routine active. Allow **Microphone** when asked
   (and **Accessibility**, if you use window placement).
4. Clap twice.

Routines can also be switched from the menu bar. Data lives in
`~/Library/Application Support/com.vibe.app/routines.json`.

## How it works

```
mic (cpal) → clap detector → double-clap matcher → actions → window placement (AX API)
```

- Rule-based detection: adaptive noise floor, FFT spectral flatness, decay
  gating. Tuned for zero false positives against typing, speech, and music.
- Clap-to-first-action latency is ~150 ms.
- Audio capture, detection, and action execution run on separate threads.

## Build from source

Requires Rust (stable), Node.js, pnpm.

```bash
pnpm install
pnpm tauri dev      # development
pnpm tauri build    # .app + .dmg
```

## Docs

- [`spec/prd.md`](spec/prd.md) — product requirements
- [`spec/history.md`](spec/history.md) — development journal
- `poc/*` branches — standalone proofs of concept, kept unmerged as reference

## License

MIT © team-poem
