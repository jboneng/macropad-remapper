# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed
- **Macros now work.** They used record type 5, which the keypad silently ignores, so
  the key kept its old action. Macros are now written like the vendor tool's delay
  page: a normal key record with a delay before each key. Confirmed on a 514C:8850
  (`h`, one second's pause, `i`).
- **Changing keys and LEDs in one Write now keeps the LEDs.** The save command used to
  be sent after the LED frames, which made the keypad throw the new LED settings away.
  The vendor tool has the same bug. The save is now sent before the LEDs, and both
  survive a replug.

### Added
- **Replug hint.** After saving keys, the keypad stops updating its LEDs until it is
  unplugged and plugged back in. The settings are stored, but the keys may stay dark
  until then. This is firmware behaviour; neither a longer pause nor a separate LED
  write helps. The app and CLI now say so after any Write that changed keys.

### Changed
- **LED effects have names:** Off, Steady, Light on press, Ripple, Wave and Gradient,
  identified on real hardware. The app shows a short description of the selected
  effect, and the user guide has a table.
- **Protocol notes:**
  - `EF EF` enters firmware-update mode; it is not a factory reset.
  - The knob slot order (left, press, right) is confirmed.
  - Media and mouse writes are confirmed on hardware.
  - The layout presets are documented.

## [0.3.1] - 2026-11-12

### Changed
- **The app draws known keypad models the way they really look.**
  - Layouts come from the vendor tool's own layout pictures: 3 keys + 1 knob, 5 keys,
    6 keys, 6 keys + 2 knobs, 9 keys + 3 knobs, 12 keys + 2 knobs and 15 keys + 3 knobs.
  - Pads with knobs beside the keys show them stacked on the right.
  - 16 keys + 3 knobs (the tested 514C:8850) stays 4 × 4 with the knobs on top.
- **Unknown sizes get more natural grids.** For example 5 keys in one row, 10 as 5 × 2 and
  15 as 5 × 3, instead of always 3 or 4 columns. A note says the layout is a guess.
- **The keypad is centred.** Knobs and keys are centred on each other, and keys shrink to
  fit narrow windows instead of running off the edge.

## [0.3.0] - 2026-11-12

### Added
- **Per-app layer files.** A YAML file can now describe a single layer (e.g. `lightroom.yaml`
  with an optional `name:`) and be loaded into *any* layer, so one person can put Lightroom
  on layer 2 and another on layer 1.
  - **App:** **Load into layer N…** and **Save layer N…** next to the layer buttons. A
    **Name** field labels the layer (e.g. **2 · Lightroom**). **Open** also accepts layer
    files and setup files.
  - **CLI:**
    - `upload <layer-file> --layer N` loads a layer file into a layer.
    - `dump --layer N --name <name>` exports one layer.
  - **Setup files** combine layer files with `layers: [ { file: lightroom.yaml }, … ]`.
    Paths are relative to the setup, and inline `slots`/`leds`/`name` override the file.
  - **Examples:**
    - `examples/layers/negpy.yaml` covers NegPy: temperature, CMY filtration, ISO-R grade,
      and shadow, print and highlight density on keys; preflash, diffusion and contrast mask
      on the knobs. NegPy actions without a default key use F13–F24.
    - `examples/layers/lightroom.yaml` covers Lightroom Classic shortcuts.
    - `examples/layers/media.yaml` covers media and volume keys.
    - `examples/setup.yaml` combines them.

### Changed
- **Misspelled field names are rejected.** For example `slot:` instead of `slots:` is now
  an error instead of being silently ignored.
- **Loading a profile in the app is all-or-nothing.** A bad slot no longer leaves the
  editor half-loaded.

## [0.2.1] - 2026-11-12

### Fixed
- **LED colours and modes now survive unplugging the keypad.** Previously they changed
  straight away but reset to factory on the next replug. Writes now follow the vendor
  tool exactly:
  - The LED block is sent for **all three layers**, not just the changed one.
  - **No commit** follows an LED-only change. The commit command made the keypad restore
    its stored LED settings.
  - Confirmed on a 514C:8850.
- **One commit per write.** Key records are now written in one batch with a single commit
  at the end, instead of a commit after every layer.

## [0.2.0] - 2026-11-12

First version tested against a real keypad (514C:8850, 16 keys + 3 knobs).

### Changed
- **Renamed the project to Macropad Remapper.**
  - Binaries are now `macropad-remapper` and `macropad-remapper-gui`.
  - Crates are now `macropad-remapper-proto` and `macropad-remapper-device`.
  - The udev rule is now `99-macropad-remapper.rules`.
  - Backups are named `macropad-remapper-backup-<time>.yaml`.
- **Redesigned the desktop app.**
  - The keypad is drawn as it looks, with knobs on top and keycaps that show their action
    and glow in their LED colour.
  - Each knob has clickable zones for turn left (↺), press and turn right (↻).
  - A card-style inspector replaces the dropdowns with buttons, for media keys, mouse
    buttons, the wheel and modifiers.
  - **Record shortcut** captures a key combination by pressing it.
  - Lighting is edited next to the key: a colour picker, preset swatches and
    "use this colour for every key".
  - The Write button shows the number of pending changes, and confirmation uses a modal
    dialog.
  - A status chip shows keypad vs. simulator, a start screen offers Connect or the
    simulator, and there is a new dark theme.
  - New launch options: `--connect` and `--simulate KEYS,KNOBS`.
- **Editing a key keeps record byte 4.** Byte 4 (meaning unknown, set on some factory
  slots) is kept while the action type stays the same, so unchanged keys are no longer
  shown as custom/raw.

### Fixed
- **Handshake parsing.** The reply is `03 FB keys knobs generation`; the decompiled
  offsets were wrong. Without this fix the keypad was not detected.
- **Device selection.** The app now talks only to the vendor-defined HID interface (usage
  page `FF00`). On Windows, writes to the keyboard interface fail with "Incorrect
  function", and that error used to hide the real result.

### Added
- **Hardware confirmation** (recorded in `docs/protocol.md`):
  - record byte 1 = slot and byte 2 = layer;
  - the LED reply layout;
  - a key write followed by a commit.
- **Read-only diagnostic examples:** `probe` (HID report descriptors), `handshake` and
  `rawquery` (sends FA/FB query frames only).
- **Documentation:** a user guide (`docs/user-guide.md`) and this changelog.

## 0.1.0 - 2026-11-12 (unreleased)

Initial version, built from static analysis of the vendor tool `MINI_KEYBOARD.exe`.

### Added
- **Protocol documentation:** `docs/protocol.md` covers handshake, layer read, record
  write, commit, LED read/write and layout select.
- **`proto` crate:** frame builders, the 64-byte key record codec (keys, media, mouse,
  timed macro, plus lossless raw passthrough), HID key/media name tables, and the YAML
  profile format.
- **`device` crate:**
  - `hidapi` transport and a simulated keypad;
  - diff-only writes, planned and validated before anything is sent;
  - dry-run mode;
  - legacy (generation < 10) firmware is read-only.
- **CLI** with `list`, `info`, `dump`, `upload` (automatic backup, `--dry-run`), `led`,
  `layout` (gated) and `keys`.
- **First desktop app (egui).**
- **Packaging:** Linux udev rule, cross-platform CI, example profile, and MIT/Apache-2.0
  licences.

[0.3.1]: https://github.com/jboneng/macropad-remapper/releases/tag/v0.3.1
[0.3.0]: https://github.com/jboneng/macropad-remapper/releases/tag/v0.3.0
[0.2.1]: https://github.com/jboneng/macropad-remapper/releases/tag/v0.2.1
[0.2.0]: https://github.com/jboneng/macropad-remapper/releases/tag/v0.2.0
