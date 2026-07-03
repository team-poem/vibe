<div align="center">

# 👏👏 V.I.B.E

### Clap twice. Your workspace appears.

**V.I.B.E** is a macOS app that turns a double clap into your entire work
setup — your apps launch, your tabs open, and every window snaps into the
screen layout you designed. No keyboard, no mouse, no dock diving.

![macOS](https://img.shields.io/badge/platform-macOS-black?logo=apple)
![Made with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB?logo=tauri&logoColor=white)
![License](https://img.shields.io/badge/license-MIT-green)
![Status](https://img.shields.io/badge/status-MVP-ff8a4c)

<!-- demo: hero gif — double clap → workspace assembling itself -->

</div>

---

## Your morning, before and after

**Before:** power on → open Cursor → open Chrome → find yesterday's tabs →
drag windows around → find the playlist → *finally* start working.

**After:** power on → 👏👏

V.I.B.E waits silently in your menu bar from the moment your Mac starts.
Two claps and your workspace assembles itself — in under a second.

## Features

**👏 A trigger you always carry**
No hotkey to remember, no widget to find. The detection engine reads the
built-in mic with an adaptive noise floor and spectral analysis — it knows
the difference between your claps and your typing, your voice, or your
music. All processing stays on-device.

**🧩 Routines for every mode**
*Deep work. Chart watching. Music night.* Build a routine for each — apps to
launch, URLs to open, in the order you want. Switch the active routine right
from the menu bar.

**🖥️ Your screen, pre-arranged**
Design your layout on a monitor mockup, just like display settings: split
the screen into halves, thirds, or quadrants and drop each app or page into
its region. When the routine fires, windows don't just open — they land
exactly where they belong.

**⚡ Faster than you can sit down**
From clap to first action in well under 300 ms. Cold-launching apps never
block the rest of the routine.

**🔒 Nothing leaves your Mac**
No account. No sign-up. No server. Your routines live in a local file and
your microphone audio is analyzed in memory, on-device, and never stored or
sent anywhere.

**📜 Know what happened**
Every run is logged with per-action results. If something fails, you'll see
exactly which action and why.

## Download

🚧 **Landing page & signed builds coming soon.**
Until then, you can [build it from source](#build-from-source) in a couple
of minutes.

## First run in 60 seconds

1. Open **V** in the menu bar → **Show settings**.
2. Create a routine — add *Launch app* and *Open URL* actions.
3. Pick a split (2 / 3 / 4) and assign each action a region on the monitor
   mockup.
4. Hit **Set active**, allow the **Microphone** prompt (and
   **Accessibility**, if you use window placement).
5. Turn on *Auto-start on login*, close the window, and clap twice. 👏👏

> Tip: a natural, quick double clap — two claps within about half a second.

---

<details>
<summary><b>Build from source</b></summary>

Requirements: macOS, Rust (stable), Node.js + pnpm.

```bash
git clone https://github.com/team-poem/vibe.git && cd vibe
pnpm install
pnpm tauri dev      # development
pnpm tauri build    # V.I.B.E.app + dmg
```

Routine data lives in `~/Library/Application Support/com.vibe.app/routines.json`.

</details>

<details>
<summary><b>How it works</b></summary>

```
mic (cpal) ──► streaming clap detector ──► double-clap matcher ──► action runner
                adaptive noise floor        interval + similarity     open apps/URLs
                FFT spectral flatness       gates                         │
                                                                 window placer (AX API)
```

- Audio capture, detection, and action execution run on separate threads —
  a busy UI can never delay a trigger.
- Actions launch first, then windows are awaited and placed, so one slow
  app never holds up the rest.
- Fixed-size windows are moved without resizing; URL actions open a fresh
  browser window identified by diffing the window list.

**Stack:** Tauri 2 (Rust) · cpal + rustfft DSP · macOS Accessibility API ·
React + TypeScript.

</details>

<details>
<summary><b>Project docs & development</b></summary>

- [`spec/prd.md`](spec/prd.md) — product requirements.
- [`spec/history.md`](spec/history.md) — development journal, from first PoC
  to each shipped feature.
- `poc/*` branches — six self-contained proofs of concept, preserved
  unmerged as reference.
- `spec/code/` — coding conventions (Rust & frontend) enforced across the
  repo.

Branch model: `main` (releases) ← `dev` ← `feat/*`.

</details>

## Roadmap

- [ ] Landing page & signed, notarized builds
- [ ] Clap sensitivity & interval tuning UI
- [ ] Multi-monitor layouts
- [ ] More actions — scripts, Shortcuts, music & volume

---

<div align="center">

MIT © team-poem — <em>Made to be heard.</em> 👏👏

</div>
