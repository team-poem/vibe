# V.I.B.E

> **V.I.B.E (Volume-Initiated Background Establisher)** — clap twice, and your
> workspace appears. A macOS menu bar app that detects a double clap and
> instantly launches your apps, opens your URLs, and snaps every window into
> a screen layout you designed.

**Turn on your Mac → clap twice. That's the whole workflow.**

## The Problem

Starting a focused work session means repeating the same small setup steps
every time: opening apps, launching browser tabs, arranging windows,
starting music. Individually trivial — together, real friction before you
can actually begin.

- **Repetitive manual setup** every time you sit down.
- **Context switching** before entering a focused state.
- **Fragmented automation** across shortcuts, scripts, and launchers.

## What V.I.B.E Does

| | |
|---|---|
| 👏 **Double-clap trigger** | A rule-based audio engine (adaptive noise floor, spectral analysis, decay gating) detects two claps from the built-in mic — while ignoring typing, speech, and music. Zero false positives in tuning tests. |
| 🧩 **Routines** | Combine actions — launch apps, open URLs — into named routines. One routine is *active* at a time and fires on the clap. |
| 🖥️ **Window layout** | Assign each action a screen region on a monitor mockup — split the screen in halves, thirds, or quadrants — and windows snap into place as they open, Rectangle-style, automatically. |
| 🎛️ **Menu bar native** | Lives in the menu bar (no Dock icon). Switch the active routine, pause detection, or toggle launch-at-login without opening a window. |
| 📜 **Execution log** | Every run is recorded with per-action results; failures name the exact action and reason. |
| 🔒 **Fully local** | No account, no login, no server. Audio never leaves the machine; all analysis and data stay in local files. |

## How It Works

```
mic (cpal) ──► streaming clap detector ──► double-clap matcher ──► action runner
    │              adaptive floor              interval +               open -a / URL
    │              FFT flatness                similarity gates             │
    └── audio thread ── detection thread ── event worker ── window placer (AX API)
```

- **Fast path:** clap-to-first-action target is under 300 ms; measured
  dispatch is ~130–160 ms.
- **Two-phase execution:** all actions launch first, then windows are
  waited on and placed — a slow cold launch never delays the next action.
- **Resilient placement:** fixed-size windows are moved without resizing;
  URL actions get their own fresh browser window, identified by diffing the
  window list.

## Getting Started

Requirements: macOS, Rust (stable), Node.js + pnpm.

```bash
pnpm install
pnpm tauri dev      # run in development
pnpm tauri build    # produce V.I.B.E.app / dmg
```

On first run:

1. Click the **V** icon in the menu bar → **Show settings**.
2. Build a routine: add *Launch app* / *Open URL* actions, pick a split
   layout, and assign regions on the monitor mockup.
3. **Set active**, save, and grant the two permissions when prompted:
   - **Microphone** — for clap detection.
   - **Accessibility** — only if you use window placement.
4. Clap twice. 👏👏 (150–600 ms apart — a natural quick double clap.)

All routine data lives in
`~/Library/Application Support/com.vibe.app/routines.json`.

## Tech Stack

- **Shell:** Tauri 2 (Rust) — chosen over Electron for background footprint
  and trigger-to-action latency.
- **Audio:** `cpal` capture + custom DSP (RMS, adaptive EMA floor,
  `rustfft` spectral flatness) on a dedicated thread.
- **Window control:** macOS Accessibility (AXUIElement) API via safe Rust
  wrappers.
- **UI:** React + TypeScript.

## Project Docs

- [`spec/prd.md`](spec/prd.md) — product requirements.
- [`spec/history.md`](spec/history.md) — full development journal, from the
  first PoC to each shipped feature.
- `poc/*` branches — five self-contained proofs of concept (audio capture,
  clap detection, double-clap matching, action latency, tauri shell, window
  layout), preserved unmerged as reference.
- `spec/code/` — coding conventions enforced across the repo.

## Roadmap

- Sensitivity & clap-interval tuning UI
- Multi-monitor layouts
- More actions: scripts, Shortcuts, music & volume control
- Signed, notarized builds with verified launch-at-login
