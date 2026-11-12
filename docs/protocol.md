# MINI_KEYBOARD macropad HID protocol

Recovered by static reverse engineering of the vendor configurator
`MINI_KEYBOARD.exe` (Qt5/MinGW, 2025-06-22,
SHA-256 `6497dc125af85c6a39e94c57384262ff0fa30748f11c40e482e65030097fc247`)
with Ghidra (via REA). No vendor code is included in this project.

Confidence legend: **H** = confirmed on real hardware (514C:8850, 2026-11-12),
**O** = observed in decompiled code, **I** = inferred,
**?** = unconfirmed (verify with a USB capture before relying on it).

## Hardware observations (514C:8850, 16 keys + 3 knobs, generation 11)

- **Interfaces.** Interface 0 is the vendor collection (usage page `FF00`, report ID 3,
  64-byte input and output). Interface 1 holds the keyboard, mouse and consumer
  collections, and Windows rejects writes to it.
- **Read layer.** `FA 19 00 01` (25 records, layer 1) returns exactly 25 replies.
  - Each reply has the form `03 FA slot layer type b4 count delay_hi delay_lo code …`.
  - So `rec[1]` = slot (1..25) and `rec[2]` = layer (1..3).
  - `rec[4]` is `01` on some factory slots. The vendor app never sets it for
    keyboard, media or macro actions; it only keeps what it read. Its **Clear**
    button zeroes `rec[4..63]` (O). So the `01` comes from the firmware's
    defaults, and its meaning (if any) is known only to the firmware (?).
- **Read LEDs.** `FA B0 00` returns `03 FA mode RGB×16`. The factory colours equal the
  vendor tool's built-in default table.
- **Write key.** `FD 0A 01 01 00 01 00 00 04` followed by `FD FE FF` set layer 1 slot 10 to
  `a`. It read back correctly, and pressing the key typed `a`.
- **Write LEDs.** LED settings persist across a replug only when the keypad receives the
  LED block for **all three layers** (`FE B0 00`, `FE B0 01`, `FE B0 02`) and **no commit
  follows**.
  - Sending `FD FE FF` after the LED frames makes the keypad restore its stored LED
    settings: the colours change live but reset on replug.
  - This matches the vendor tool, which commits only when key records were written.
  - When keys and LEDs change together, the vendor's save routine sends **all dirty
    records → `FE B0` for layers 0, 1, 2 → one `FD FE FF` commit → sleep 200 ms** (O,
    traced in the save handler `on_confirmButton_clicked`). On hardware that order
    **loses the LED change**: the keys were saved, but the keypad read back its old
    LED settings (**H**). So the vendor tool has this bug too.
  - macropad-remapper sends **records → commit → 200 ms → `FE B0` × 3, no commit**.
    With that order both the keys and the LEDs were saved and survived a replug (**H**).
    However, **after any commit the keypad stops updating its live LEDs until it is
    replugged** (**H**). The LEDs go dark and don't react to key presses. This happens
    even with a 1.2 s pause before the LED frames, and even for a later, separate
    LED-only write. All those LED settings are stored, and they show correctly after a
    replug. LED-only writes made after a fresh plug-in update the display immediately.
    The app tells the user to replug after any write that changed keys.
- **Mouse and media writes** (**H**): `media: volup` raised the Windows volume, and
  `mouse: { wheel: 1 }` scrolled up.
- **Macros** (**H**): a type-5 record is silently ignored, and the slot keeps its old
  action. A **type-1** record with delays works: `FD 03 01 01 00 02 | 00 00 0B | 03 E8 0C`
  typed `h`, waited one second, then typed `i`. Each step's delay is waited *before*
  its key is sent. This matches the vendor UI, whose delay page only edits type-1
  records.
- **Knobs** (**H**): the left knob uses slots 17/18/19, the middle knob 20/21/22 and
  the right knob 23/24/25. Within each triple: turn left (CCW), press, turn right (CW).
- **LED modes** (**H**, set on all three layers):

  | Mode | Effect |
  |---|---|
  | 0 | Off |
  | 1 | Steady: every key glows in its own colour |
  | 2 | Light on press: a key lights in its colour only while pressed |
  | 3 | Ripple: the pressed key lights, then the light spreads to its neighbours |
  | 4 | Wave: steady colours with a fading wave from top-left to bottom-right |
  | 5 | Gradient: built-in green-to-amber across the columns; ignores the key colours |

## Device identification (O)

| Vendor ID | Product IDs (tried in this order) |
|-----------|-----------------------------------|
| `0x1189`  | `8842 8840 8830 8831 8832 8833 8850 8851` |
| `0x514C`  | same list (fallback vendor)       |

- The vendor app enumerates and opens the first entry with `interface_number == 0` (O).
- Older `1189:8840/8842/8890` boards that do not answer the handshake below use a
  different protocol. That protocol is handled by
  [ch57x-keyboard-tool](https://github.com/kriomant/ch57x-keyboard-tool).

## Transport (O)

- Output: 65-byte HID output report. Byte 0 is report ID `0x03`, followed by 64 payload bytes.
- Input: 64-byte reads. In the vendor app, `resp[0]` is the report ID.
- In the tables below, `p[n]` means payload byte *n* (so frame byte *n+1*).

## Commands

| Name | Payload (`p[0..]`) | Reply |
|------|--------------------|-------|
| Handshake (**H**) | `FB FB FB` | `03 FB keys knobs generation 00…`. Example from a real 514C:8850: `03 fb 10 03 0b` = 16 keys, 3 knobs, generation 11. **Generation < 10 → legacy record layout.** (Ghidra's stack offsets for this reply were wrong; hardware wins.) |
| Read layer (O) | `FA n 00 L` (legacy: `FA n knobs L`) | `n` replies, one per slot 1..n. Record bytes `rec[1..59]` = `r[2..60]` |
| Write record (O) | `rec[0..63]` | none |
| Commit (O) | `FD FE FF` | none. The vendor app sleeps 200 ms afterwards |
| Read LEDs (O) | `FA B0 L` (L = layer 0..2) | `r[2]` = mode, `r[3..50]` = 16 × RGB |
| Write LEDs (**H**) | `FE B0 L mode RGB×16` | none. Send all 3 layers (L = 0, 1, 2) and **no commit** for an LED-only change; a commit makes the LEDs reset on replug |
| Select layout (O) | `FC FC keys knobs flag` | none. Handler `Set_Keyboard_Ver_SLOT(int)`. Afterwards the vendor loads default mappings, writes them (records + commit), sleeps 200 ms and re-handshakes. **Dangerous** |
| Enter firmware-update mode (O) | `EF EF` | none. Handler `KB_UpData_SoftWare()`, wired to the "点击进入 升级模式" ("click to enter upgrade mode") button. Puts the keypad into its bootloader; **not** a factory reset. **Never sent by this tool** |

The vendor's 16 layout presets for `FC FC keys knobs flag` (O, from the
`Set_Keyboard_Ver_SLOT` switch):

| Preset | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Keys+knobs | 2+0 | 3+1 | 4+0 | 4+1 | 5+0 | 6+0 | 6+1 | 6+2 | 9+2 | 9+3 | 11+3 | 12+2 | 12+3 | 15+3 | 4+1 | 3+0 |
| `flag` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | **1** | 0 |

- `flag` is only `1` for the second 4 keys + 1 knob board (`4+1_2KEY` in the vendor's
  list), so it selects between two different 4+1 hardware variants (O).
- There is no 16 keys + 3 knobs preset; the tested 514C:8850 reports 16+3 itself.
- All vendor write commands (`FC`, `EF`, records, LEDs) are gated on a "device
  connected" flag (O).

- The vendor app sends the commit only after a batch of record writes, never for an
  LED-only change (O, **H**).
- **Read count (O).** On generation ≥ 10 the vendor always sends `FA 19 00 L` (**25**
  records) for L = 1, 2, 3, whatever the keypad size. It stores reply *i* as slot *i*
  without looking at the reply's slot byte, and copies 59 bytes per record. Older
  firmware gets `FA 0F 03 L` (15 key records, then 3 × knobs extra records).
  This tool requests `keys + 3 × knobs` (= 25 on the 16+3 pad) and stops early if a
  read times out.
- Slot 0 is unused. Slots 1..keys are keys, and each knob takes the next three slots,
  in the order turn left (CCW), press, turn right (CW). Knobs are numbered left to
  right (**H**, 514C:8850). The vendor UI draws them as `KN_LEFT` / `KN_IN` / `KN_RIGHT`.
- The vendor's internal button-to-slot table stops at 25, so 25 slots is the
  most any supported keypad uses (O).
- Layers are numbered 1..3 in the `FA` read and 0..2 in the LED commands (O).

## Key record, generation ≥ 10 (64 bytes)

| Offset | Meaning |
|--------|---------|
| 0 | `0xFD` (O, initialised by the app; not part of the read-back) |
| 1, 2 | Slot number (1-based), layer number (1-based) (**H**) |
| 3 | Action type (O): `1` keyboard (also timed sequences, via the step delays), `2` media, `3` mouse, `8` LED. The vendor's tab list also uses `5` for its delay page, but the firmware ignores type-5 writes (**H**) |
| 4 | Mouse sub-mode (O): `1` buttons/wheel, `4` "special" (codes 1..5). On non-mouse factory slots it is sometimes `1` (**H**, meaning ?). macropad-remapper keeps it while the type is unchanged |
| 5 | Number of steps used (O) |
| 6 + 3n, 7 + 3n | Step *n* delay, big-endian, ms (O; UI range 50..60000 for macros; the UI's default per-step delay is 50 ms = `00 32`) |
| 8 + 3n | Step *n* code (O) |

Up to 18 steps fit in a record (O).

### Code encoding (O)

- **Keyboard keys**: standard USB HID keyboard usage IDs (A = `0x04` … `0x64`).
- **Modifiers** are separate steps with pseudo-codes:
  `F1` LCtrl, `F2` LShift, `F3` LAlt, `F4` LGui, `F5` RCtrl, `F6` RShift, `F7` RAlt, `F8` RGui.
  - `Ctrl+C` = steps `[F1, 06]`.
  - Shifted symbols such as `!` = `[F2, 1E]`.
- **Media** (type 2): a 16-bit HID consumer usage split over two steps, `code[0]` = low byte and
  `code[1]` = high byte. Examples: Play/Pause `0x00CD`, Mute `0x00E2`, Vol+ `0x00E9`,
  Vol− `0x00EA`, Next `0x00B5`, Prev `0x00B6`, Stop `0x00B7`, Calculator `0x0192`,
  Mail `0x018A`, WWW Home `0x0223`, Back `0x0224`, Forward `0x0225`, Refresh `0x0227`.
- **Mouse** (type 3, `rec[5]=4`): `code[1]` (=`rec[11]`) buttons (1 left, 2 right, 4 middle),
  `code[4]` (=`rec[20]`) wheel (`01` up, `FF` down), and an optional `code[0]` modifier pseudo-code
  (F1/F2/F3 = Ctrl/Shift/Alt + wheel). The vendor's mouse page offers exactly: left,
  right and middle click, wheel up/down, and Ctrl/Shift/Alt + wheel up/down (O).
- **Mouse "special"** (`rec[4]=4`, codes 1..5): the vendor labels these
  **"Mouse/Swipe screen"**: 点赞 *Like*, 向左划屏 *swipe left*, 向右划屏 *swipe right*,
  向上划屏 *swipe up*, 向下划屏 *swipe down* (O). These are phone-style gestures
  (e.g. double-tap to like). The code-to-gesture order is assumed to follow the UI
  order, Like = 1 … Down = 5 (I).
- **Media page** in the vendor app (O): next, previous, stop, play/pause, mute,
  volume ±, screen brightness ±, bass ±, treble ±, browser back/forward/refresh,
  "My Computer", and open media player / calculator / browser / mail.

## Key record, legacy generation (< 10)

- `rec[0]=FE`, `rec[1]` = id, `rec[2]` = layer+1, `rec[3]` = type, `rec[9]` = count.
- Steps are 2 bytes starting at `rec[10]` (`modifier-mask, keycode`, with standard HID modifier bits).
- Only 49 bytes are read back per record.
- Support in this project is **experimental**.

## LED modes

- Generation ≥ 10: mode `0..5`. The vendor app labels them only "LED模式0" …
  "LED模式5" ("LED mode 0–5") (O). The effects were identified on hardware (**H**):
  0 off, 1 steady, 2 light on press, 3 ripple, 4 wave, 5 gradient. See the table under
  *Hardware observations*. The mode is stored per layer (one byte in each `FE B0 L`
  frame), so a layer on mode 0 is dark even if other layers are lit.
  Per-key colours are RGB triples.
- Legacy: one byte, `mode << 4 | colour`, defaulting to high nibble `5`.
