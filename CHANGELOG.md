# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

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

[0.2.0]: https://github.com/jboneng/macropad-remapper/releases/tag/v0.2.0
