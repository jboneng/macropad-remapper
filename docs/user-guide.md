# Macropad Remapper — User guide

This guide is for anyone with one of the small "MINI_KEYBOARD" macropads who wants to
change what its keys and knobs do. You don't need to be a programmer.

**Contents**

1. [Before you start](#1-before-you-start)
2. [Using the desktop app](#2-using-the-desktop-app)
3. [Using the command line](#3-using-the-command-line)
4. [Profile files](#4-profile-files)
5. [Backups and undo](#5-backups-and-undo)
6. [Troubleshooting](#6-troubleshooting)
7. [Reference: key and media names](#7-reference-key-and-media-names)

---

## 1. Before you start

### What you need

- A supported keypad. It identifies itself over USB as vendor `1189` or `514C` with a product
  ID in the 88xx range. The tested model is `514C:8850` (16 keys, 3 knobs).
- The two programs: `macropad-remapper-gui` (desktop app) and `macropad-remapper`
  (command line). Download them from the project's Releases page, or build them from
  source (see the README).

### Platform notes

- **Windows:** nothing to install. If Windows SmartScreen warns about an unknown publisher,
  choose *More info → Run anyway* (the binaries are not code-signed).
- **Linux:** install the udev rule once, then unplug and replug the keypad:
  ```sh
  sudo cp 99-macropad-remapper.rules /etc/udev/rules.d/
  sudo udevadm control --reload && sudo udevadm trigger
  ```
- **macOS:** the first time you connect, macOS may ask to allow *Input Monitoring*. Allow it
  in *System Settings → Privacy & Security*.

### Two things to know about your keypad

- **Layers.** The keypad stores **3 layers**: three complete sets of key assignments. Your
  keypad has its own way of switching between them, often a key combination or a small
  button; check its manual.
- **Knobs.** Each knob has **three actions**: turn left (↺), press (●) and turn right (↻).

---

## 2. Using the desktop app

### Connect

1. Plug in the keypad.
2. Start **Macropad Remapper** (`macropad-remapper-gui`).
3. Click **Connect keypad**.

The app reads the keypad's current settings and draws it. A green chip in the top right
says **Keypad connected · 16 keys · 3 knobs** (or whatever your model has).

Connecting only *reads* from the keypad. Nothing changes until you press **Write**.

> Want to look around first? Click **Try the simulator** on the start screen, or start the
> app with `macropad-remapper-gui --simulate 16,3`. The chip turns orange and nothing is
> ever sent to real hardware.

### The screen

- **Top bar**
  - **Connect:** connect to the keypad.
  - **Reload:** re-read the keypad.
  - **Open / Save:** load or save profile files.
  - **Write:** send your changes to the keypad.
- **Left**
  - **LAYER 1 2 3** chooses which layer you are editing.
  - Below it, your keypad: knobs along the top, keys underneath. Each key shows what it
    currently does, and its glow is its LED colour.
- **Right:** the inspector for whatever you selected, with an **Action** card and a
  **Lighting** card.

### Change what a key does

1. Pick the layer at the top left.
2. Click the key.
3. In **Action**, choose one of the types below.

| Type | What it does | How to set it |
|---|---|---|
| **None** | The key does nothing | — |
| **Shortcut** | Presses keys together, like `Ctrl+C` | Click **Record shortcut** and press the combination on your normal keyboard, or type it in the box, e.g. `ctrl-shift-esc` |
| **Media** | Volume, play/pause, next track, calculator… | Click one of the buttons |
| **Mouse** | Click or scroll | Choose buttons, scroll direction and an optional held modifier (Ctrl + scroll = zoom in most apps) |
| **Macro** | Taps keys one after another with delays | Add steps; for each step pick a key and the delay (ms) to wait before it |

Repeat for as many keys as you like. The **Write** button counts your pending changes.

> **Recording tips**
> - The Windows key and some system shortcuts (like `Alt+Tab`) are grabbed by the operating
>   system before the app sees them. Type those instead, e.g. `win-d`.
> - Handy shortcuts for a macropad are `f13` … `f24`. No normal keyboard has them, so no app
>   uses them by default, and you can bind them freely in OBS, Discord, games and so on.

### Knobs

Click **↺** to set what turning left does, the **knob itself** for pressing, and **↻** for
turning right. Then choose an action exactly like a key. A common setup is
↺ = *Volume −*, ● = *Mute*, ↻ = *Volume +*.

> The ↺ / ● / ↻ order is confirmed on the 16-key + 3-knob model. If a knob on another
> model turns the wrong way, swap the ↺ and ↻ actions.

### Lighting

In the **Lighting** card:

- **Effect (whole layer):** each layer has its own effect.

  | Mode | Name | What it does |
  |---|---|---|
  | 0 | Off | All LEDs off |
  | 1 | Steady | Every key glows in its own colour |
  | 2 | Light on press | A key lights up in its colour only while pressed |
  | 3 | Ripple | The pressed key lights up and the light spreads to its neighbours |
  | 4 | Wave | Steady colours with a fading wave from top-left to bottom-right |
  | 5 | Gradient | Built-in green-to-amber gradient; ignores the key colours |

  Remember that the effect belongs to a layer: if the keys stay dark, check which layer
  the keypad is on.
- **Key N colour:** pick a colour for the selected key, or click a preset dot.
- **Use this colour for every key:** copies the colour to all keys on this layer.

### Write to the keypad

1. Click **Write N change(s)**.
2. Confirm.

The app then:

1. Re-reads the keypad and **saves a backup** to your home folder
   (`macropad-remapper-backup-<number>.yaml`).
2. Writes only the keys that changed.
3. Reads everything back, so the screen shows what's really on the keypad.

The status bar at the bottom tells you how many keys were written and where the backup
is. Changes take effect immediately and survive unplugging.

> **Lighting after changing keys:** once you write key changes, the keypad stops
> updating its LEDs (they may go dark) until you **unplug it and plug it back in**.
> Your settings are saved; replugging just makes the keypad show them. Lighting-only
> changes show straight away.

### Save and open profiles

- **Save** writes everything you see (all layers and LEDs) to a `.yaml` file.
- **Open** loads a profile into the editor. It is not written to the keypad until you
  press **Write**, so you can review it first. Opening a *layer file* puts it into the
  layer that is currently selected.

### Per-app layers (layer files)

You can keep one file per app, e.g. `lightroom.yaml` or `negpy.yaml`, and put each on
whichever layer you like. Someone else can use the same files on different layers.

- **Save the current layer as a file.**
  1. Pick the layer.
  2. Type a **Name** (e.g. *Lightroom*) next to the layer buttons.
  3. Click **Save layer N…**.
- **Put a layer file onto a layer.**
  1. Pick the target layer.
  2. Click **Load into layer N…** and choose the file.
  3. Click **Write**.

  The layer's name appears on its button (e.g. **2 · Lightroom**).

See [Layer files](#layer-files) for the file format, and `examples/layers/` for a
ready-made Lightroom layer.

---

## 3. Using the command line

Open a terminal (on Windows: PowerShell or Command Prompt) in the folder containing
`macropad-remapper`.

### Is the keypad there?

```sh
macropad-remapper list    # USB interfaces of any supported keypad
macropad-remapper info    # asks the keypad what it is
```

`info` prints, for example:

```
keys:       16
knobs:      3
generation: 11
slots:      1..=25 (keys first, then 3 per knob)
```

### Save the current setup

```sh
macropad-remapper dump -o my-keypad.yaml
```

Do this once and keep the file. It's your factory backup.

### Change it

Edit `my-keypad.yaml` in any text editor (see [Profile files](#4-profile-files)), then:

```sh
macropad-remapper upload my-keypad.yaml --dry-run   # show exactly what would be sent
macropad-remapper upload my-keypad.yaml             # do it
```

`upload` first saves a backup of the current keypad to
`macropad-remapper-backup-<number>.yaml` in the current folder, then writes only the slots
that changed, for example:

```
backup of current configuration: macropad-remapper-backup-1794481977.yaml
wrote 3 key record(s) and 0 LED layer(s)
```

If the keypad already matches, it says so and writes nothing.

### Per-app layers

```sh
# Save layer 3 as a reusable layer file
macropad-remapper dump --layer 3 --name NegPy -o negpy.yaml
# Put a layer file onto any layer
macropad-remapper upload lightroom.yaml --layer 2
# Or apply a setup that combines several layer files (see Layer files below)
macropad-remapper upload setup.yaml
```

### Other commands

| Command | Purpose |
|---|---|
| `macropad-remapper led --layer 1 --mode 3 --color '#ff0000' --color '#00ff00'` | Set a layer's LED mode and per-key colours (one `--color` per key, in order). Add `--dry-run` to preview |
| `macropad-remapper keys` | List every key, modifier and media name you can use |
| `macropad-remapper layout --keys N --knobs M --i-know-what-im-doing` | Change the keypad's layout preset. **Resets its mappings.** Only needed if the keypad reports the wrong number of keys/knobs |
| `--simulate 16,3` (any command) | Use a pretend keypad instead of USB |
| `-v` (any command) | Print every command byte sent to the keypad |

---

## 4. Profile files

A profile is a plain-text YAML file. Indentation matters: use spaces, not tabs.

```yaml
layers:
  - slots:                       # layer 1
      1: ctrl-c                  # shortcut: keys pressed together
      2: ctrl-v
      3: f13
      4: { media: play }         # media key
      5: { media: volup }
      6: { mouse: { buttons: [left] } }              # mouse click
      7: { mouse: { wheel: 1, modifier: ctrl } }     # Ctrl + scroll up (zoom in)
      8:                         # timed macro: types "hi" and Enter
        macro:
          - { key: h, delay: 50 }
          - { key: i, delay: 50 }
          - { key: enter, delay: 50 }
      9: none                    # does nothing
    leds:
      mode: 1
      colors: ["#ff0000", "#00ff00", "#0000ff"]      # key 1, key 2, key 3, …

  - slots:                       # layer 2
      1: { media: mute }
```

### Slot numbers

- Slots `1` … `N` are the keys, counted left to right, top to bottom.
- After the keys come **three slots per knob**: turn left, press, turn right.
- On a 16-key, 3-knob pad:

  | Slots | Control |
  |---|---|
  | 1–16 | Keys |
  | 17, 18, 19 | Knob 1: ↺, ●, ↻ |
  | 20, 21, 22 | Knob 2: ↺, ●, ↻ |
  | 23, 24, 25 | Knob 3: ↺, ●, ↻ |

- Run `dump` to see your own keypad's numbering.

### Rules

- **Slots you leave out are not touched.** A profile can be just the three keys you care
  about.
- **Shortcuts** are written with `-` or `+` between keys: `ctrl-shift-esc`, `alt+f4`, `win-d`.
  Single symbols such as `!`, `@` and `?` work on their own; Shift is added for you.
- **Macro** steps take one key (or one modifier) each. `delay` is in milliseconds (0–60000)
  and is waited *before* that step. At most 18 steps.
- **`raw: …`** appears in dumps for settings the tool doesn't understand yet. Leave it as it
  is; it will be written back unchanged.
- **The `device:` block** at the top of a dump is informational and ignored on upload.
- **Misspelled field names** (e.g. `slot:` instead of `slots:`) are reported as errors.

### Layer files

A **layer file** describes a single layer. It has the same fields as one `layers:` entry,
but at the top level, plus an optional `name`:

```yaml
# lightroom.yaml
name: Lightroom
slots:
  1: p              # flag as pick
  2: x              # reject
  17: left          # knob 1 ↺ previous photo
  19: right         # knob 1 ↻ next photo
leds: { mode: 1, colors: ["#3ddc97", "#ff5d73"] }
```

A layer file isn't tied to any layer. You choose where it goes when you load it:
`--layer N` on the command line, or **Load into layer N…** in the app.

A **setup** is a full profile that combines layer files with `file:`. Paths are relative to
the setup file:

```yaml
# my-setup.yaml                        # a friend's setup.yaml
layers:                                # layers:
  - file: layers/negpy.yaml            #   - file: layers/lightroom.yaml
  - file: layers/lightroom.yaml        #   - file: layers/negpy.yaml
  - file: layers/media.yaml
    slots:                             # inline entries override the layer file
      16: f13
```

A layer file can't include another file. Loading a layer file only changes the slots it
lists; save complete layers with **Save layer** / `dump --layer` if you want every slot
included.

---

## 5. Backups and undo

- **Every write makes a backup first.**
  - The app saves it to your home folder.
  - The command line saves it to the folder you ran it from.
- **To undo**, upload the backup, either with `macropad-remapper upload <backup>.yaml` or
  with **Open → Write** in the app.
- **For a factory-fresh copy**, run `macropad-remapper dump -o factory.yaml` once and keep
  the file somewhere safe.

---

## 6. Troubleshooting

| Problem | What to do |
|---|---|
| **"no supported keypad found"** | Check the cable and try another USB port. Run `macropad-remapper list`; if it shows nothing, your model may have a different USB ID — please open an issue with the output of your OS's USB device list |
| **"the keypad did not answer the handshake"** | Your board may use the older protocol; try [ch57x-keyboard-tool](https://github.com/kriomant/ch57x-keyboard-tool). Also close the vendor tool if it's running, since only one program can talk to the keypad at a time |
| **Linux: "permission denied"** | Install the udev rule (see [Before you start](#1-before-you-start)) and replug |
| **"legacy firmware: read-only"** | Your keypad uses an older record format. You can dump it but not write yet |
| **"slot N is not reported by the device"** | Your profile uses a slot number your keypad doesn't have; check `info` |
| **A knob turns the wrong way** | Swap its ↺ and ↻ actions |
| **A shortcut doesn't record** | The OS grabbed it first; type it into the box instead |
| **The keypad behaves strangely after a write** | Upload your backup (see [Backups and undo](#5-backups-and-undo)) |

When reporting a problem, include the output of `macropad-remapper info` and, if relevant,
`macropad-remapper -v upload <file> --dry-run`.

---

## 7. Reference: key and media names

Run `macropad-remapper keys` for the authoritative list. Summary:

- **Modifiers:** `ctrl` `shift` `alt` `win` and the right-hand versions `rctrl` `rshift` `ralt`
  `rwin`.
  - Aliases: `control`, `cmd`, `command`, `super`, `gui`, `meta` (all = `win`); `option` = `alt`;
    `altgr` = `ralt`.
- **Letters and digits:** `a`–`z`, `0`–`9`.
- **Function keys:** `f1`–`f24`.
- **Editing and navigation:** `enter` `esc` `backspace` `tab` `space` `insert` `delete` `home`
  `end` `pageup` `pagedown` `up` `down` `left` `right` `capslock` `printscreen` `scrolllock`
  `pause` `menu`.
- **Punctuation:** `minus` `equal` `lbracket` `rbracket` `backslash` `semicolon` `quote` `grave`
  `comma` `dot` `slash`, or the characters themselves: `-` `=` `[` `]` `\` `;` `'` `` ` `` `,`
  `.` `/`.
- **Shifted symbols:** `!` `@` `#` `$` `%` `^` `&` `*` `(` `)` `_` `+` `{` `}` `|` `:` `"` `~`
  `<` `>` `?`.
- **Number pad:** `kp0`–`kp9` `kpdot` `kpenter` `kpplus` `kpminus` `kpasterisk` `kpslash`
  `numlock`.
- **Media:** `play` `stop` `prev` `next` `mute` `volup` `voldown` `brightnessup`
  `brightnessdown` `mediaselect` `mail` `calculator` `mycomputer` `wwwsearch` `wwwhome`
  `wwwback` `wwwforward` `wwwstop` `wwwrefresh` `wwwfavorites`.
- **Anything else:** a raw HID code as hex, `0x68` for a key or `{ media: 0x0223 }` for a
  media key.
