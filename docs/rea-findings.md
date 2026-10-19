# ShinetekTools static investigation — 2026-10-19

## Artifact and method

Actual extracted executable: `/tmp/opencode/lecoo_msi/app_main/ShinetekTools.exe`.
SHA-256: `89a0c7792ceff1adde158e7ef28f08ca7fa2d6945d065c7f566434e96b54ac88`.
Setup exists at `/home/borat/Downloads/Lecoo_GK301 setup v1.0.1.1(1)(1).exe`; bottle cache contains `ShinetekTools.msi`, but no installed app was found under its Program Files directories.

REA `inspect_with_objdump(file_headers)` Evidence ID: `ev_715fda67772705dd5a6da071486fd5f8a1076d53863998ecb8f395bffd895857`; confirms PE i386, entry 0x004bd0bd, unchanged digest before/after. Native Hopper deep query was blocked because another REA session already owns Linux Hopper (0a83f00d-dbcb-41f4-ba53-b03a58a76f86); it was left untouched. Our logical target was closed.

Focused current-file GNU objdump excerpts are retained in `shinetek-protocol-disassembly.txt`. Those excerpts are direct shell observations, not REA-issued Evidence IDs. No Windows app execution or hardware writes were performed.

## Observed static facts

- Feature sender 0x404cb0: pushes report length 8, buffer, device handle; calls IAT 0x64a0d8 at 0x404d48. Import table identifies that slot as `HidD_SetFeature`.
- Panel sender 0x41feb0 admits effect 0..19, forms prefix `07 FF FF`, then effect at report byte 3, clamps brightness byte 4 to maximum 5 at 0x41ff69, includes speed/selection data and final zero. Loader values 1/2/3 are mapped to wire 0/1/2 at 0x41ff71..0x41ff8f.
- Panel path computes elapsed time from shared timestamps and sleeps for `100 - elapsed` at 0x41ff1e..0x41ff47. Side sender 0x41ffd0 uses the same timestamp offsets +0x900/+0x904 and analogous wait at 0x420032..0x42005a. Both update timestamps after sending.
- Side sender admits effect 0..11 and forms `07 FF FF 20 00 00 XX 00` at 0x420060..0x420084. Its low nibble is replaced by the chosen effect; the stored high nibble is preserved.
- Reset path constructs `07 F0 AA 00 00 00 00 00` at 0x41fe76..0x41fe86 and calls a different transport routine 0x4049f0. Do not infer feature transport for reset from lighting transport.
- Packaged `dlg_main.xml` declares panel brightness 0..5, speed 0..2, side brightness 0..7. It has separate effect settings pages: speed+brightness+direction, speed+brightness, brightness alone, and brightness+loader. M3 loader and the last page speed slider have zero size. This establishes UI exposure, not exact effect-to-page dispatch.

## Implementation implications (inferences)

- Use shared command serialization and approximately 100 ms pacing between panel/side writes; rapid independent CLI processes must share the lock/pacing state if invoked from UI. Windows matching pacing is a compatibility choice, not proof that faster commands always fail on hardware.
- Some unchanged speed/direction/loader settings are expected for effects that do not expose them in the original Windows UI. A universal control should explain that behavior or be gated only after exact effect dispatch has been recovered.
- Correct prior documentation: brightness 5 is demonstrably clamped by Windows application code. Firmware clamping is unproven by this static evidence.

## Unknowns

Exact mapping of 20 effects to settings pages; safe readback of current lighting state; additional key-remap or macro protocol; full reset transport call chain. Existing Python proof-of-concept and README are historical project claims, not independent hardware verification in this pass. New unsupported raw commands should not be guessed.
