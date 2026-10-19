# OmaLecoo

Lecoo GK301 keyboard RGB lighting for Omarchy — a native bar plugin + Rust
CLI (SEMICO `1a2c:9df4`). 20 lighting effects, brightness (0–13), speed,
presets, connection diagnostics — all through real HID Feature reports that
I recovered by reverse engineering the official Windows tool.

## Install

```bash
omarchy plugin add https://github.com/OnurByte/OmaLecoo.git --enable
```

That installs and enables the bar plugin (the repo root is the plugin
folder: `manifest.json`, `BarWidget.qml`, `Panel.qml` — the layout the
[marketplace guide](https://plugins.omarchy.org/develop.html) documents).
The keyboard controller binary is separate; pick one way to provide it:

```bash
# from source (needs Rust/cargo)
cargo install --git https://github.com/OnurByte/OmaLecoo
```

```bash
# or the prebuilt binary from a GitHub release
mkdir -p ~/.local/bin
curl -fL -o ~/.local/bin/lecoo-gk301 \
  https://github.com/OnurByte/OmaLecoo/releases/latest/download/lecoo-gk301
chmod +x ~/.local/bin/lecoo-gk301
```

If the binary lives somewhere else on `PATH`, set it in the plugin's bar
settings (`binary` property). Either way, grant the HID node user access —
one sudo pass, once:

```bash
sudo bash scripts/install-udev.sh   # from inside the plugin directory
```

Installing from a plain checkout instead: run `./install.sh` from the repo;
it builds the controller, installs the binary and plugin files and takes at
most one sudo (the udev rule). Daily use never needs sudo: the rule tags the
vendor HID node with `uaccess`. If the bar keeps an older copy, run
`omarchy restart shell`.

## The story (why this exists)

I use Omarchy. I love DHH's stuff — Ruby on Rails, the *just build the thing*
energy, and now Omarchy itself — so when a cheap RGB keyboard shows up on my
desk, the last thing I want is a Windows-only control panel sitting there,
useless.

The Lecoo GK301 ships with exactly that: a Windows-only installer. Finding it
was its own quest. Distributors hide the driver behind vague "software"
buttons; I dug through store pages and support sites until I finally found
the actual download hidden in a Google Drive folder (same link the Turkish
Lecoo distributor Wawa Teknoloji hands out):

**Original Windows tool (Lecoo_GK301 setup v1.0.1.1):**
<https://drive.google.com/drive/folders/1_o3YHjDMwQCvY8Bd8kiL-gRfki8ezlaU>

So now I had the exe. Did it run under Wine/Lutris/Bottles? Nope. Crashes,
missing components, nothing. I wasn't going to reboot into Windows for a
keyboard, so I did the normal thing instead:

- ran the exe in a throwaway Windows environment once, exported its UI XMLs,
- dissected it with REA (static disassembly + evidence capture),
- recorded the real USB traffic with a usbmon capture while applying effects,
- rebuilt the confirmed protocol as a tiny Rust binary + Quickshell panel.

The result is OmaLecoo: a plugin that talks to the keyboard natively on
Omarchy — no Wine, no VM, no sudo after install.

## What it actually does (no fairy tales)

Confirmed by live usbmon capture, REA analysis of the Windows binary, and
physical observation. Firmware on the tested unit: **1.16**.

| Feature | Status |
| --- | --- |
| 20 main lighting effects | ✅ works |
| Brightness 0–13 (the Windows app only offered/clamped 0–5 — scale found via REA + live probes, user-confirmed visibly) | ✅ works |
| Speed 0–2, gaming keys M1/M2 | ✅ works |
| Presets (quiet / gaming / night / party / off / random) | ✅ works |
| Auto effect rotation, reapply on demand | ✅ works |
| Connection diagnostics + debug log | ✅ works |
| Firmware response query (`response`) | ✅ works |
| Side RGB, per-key RGB, custom colors | ❌ removed |
| On-device macro programming | ❌ never existed |

The side-LED and reset paths found in the Windows tool belong to a hidden
Mouse page and never produced a visible change on this keyboard — so they are
**not exposed**. GUI presets send exactly one main-panel packet.
Arbitrary RGB colors were *not* recoverable from the tool: its lighting
writer carries effect/brightness/speed/selection and no RGB triplet. I keep
the honesty: whatever is not confirmed is not shipped.

Recovered protocol (8-byte Feature reports, report ID 7, interface 1):

```
panel effect : 07 FF FF <effect 0-19> <brightness 0-13> <speed 0-2> <select 0-2> 00
firmware     : 07 F1 00 00 00 00 00 00 (set, then get)
```

`lecoo-gk301 response` sends the firmware query and surfaces the device's
validated reply (version, raw bytes, roundtrip time). One honest caveat: this
firmware path reports its version but not the current lighting state, so the
panel always shows the last settings sent from this computer.

## Controls

Bar icon: left click opens the panel, right click = next effect, scroll =
brightness. In the panel: search, effect grid, brightness slider (0–13,
finer than the original Windows app), speed (Slow/Medium/Fast), M1/M2 for
the Gaming effect, presets, auto cycle. Shortcuts: `n`/`p`
next/previous effect, `o` off, `r` refresh, `Esc` close. Diagnostics
(`doctor` output) appear inside the panel when you ask for them.

## CLI

```bash
lecoo-gk301 doctor              # device, paths, validated firmware reply
lecoo-gk301 response --json     # validated firmware response with timing
lecoo-gk301 status --json       # last applied settings
lecoo-gk301 panel 2 --brightness 5 --speed 1 --select 0
lecoo-gk301 preset night
lecoo-gk301 capabilities --json # truthful capability report
```

`side`, `raw` and `reset` debug commands from earlier builds were removed;
the analysis records still describe them in `docs/` for anyone continuing
the protocol work.

## Project layout

- `src/main.rs` — controller (HID transport, pacing, lock, debug log)
- repo root doubles as the plugin folder: `manifest.json`, `BarWidget.qml`,
  `Panel.qml` (Quickshell/QML bar widget + panel)
- `install.sh`, `scripts/`, `udev/` — installer and one-shot device rule
- `docs/FINAL-ANALYSIS.md` — what the captures + REA concluded
- `docs/captures/` — real usbmon pcapng + REA evidence exports
- `docs/custom-color-investigation.md` — why custom RGB is unproven
- `tests/` — offline release checks (no USB hardware required)
- `reference/` (kept local, git-ignored) — original Windows artifacts for
  future REA sessions; please don't republish Lenovo/SHINETEK binaries

## Verify

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
node tests/ui-state.cjs
bash -n install.sh scripts/install-udev.sh
```

## License

[MIT](LICENSE) — use it, fork it, ship it.
