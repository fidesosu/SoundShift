# SoundShift

A small Windows tray app that lowers the volume of chosen programs (games, browsers, ...) while they are **not focused**, and restores it when you switch back to them.

- Lives only in the system tray, no windows or console.
- Only changes the per-app volume through the Windows audio API (the same volume the Volume Mixer shows). Games and programs are never modified or injected into.
- Lightweight: it reacts to Windows focus events instead of polling, so it uses no CPU while idle.

Target: Windows 10 (also works on Windows 11).

## Usage

1. Build it (or grab a release build):
   ```
   cargo build --release
   ```
   The exe is `target\release\SoundShift.exe`.
2. Run `SoundShift.exe`. A speaker icon appears in the system tray.
3. Click the tray icon and choose **Open config**, add your programs, save, then choose **Reload config**.

### Tray menu

| Item | What it does |
|---|---|
| **Pause** | Stops ducking and restores all volumes. The icon loses its sound waves while paused. |
| **Reload config** | Re-reads the config file. |
| **Open config** | Opens the config file in your default editor for `.toml` files. |
| **Exit** | Restores all volumes and quits. |

### Config

Location: `%APPDATA%\SoundShift\config.toml` (created with an example on first run).

```toml
# Volumes are 0.0 - 1.0
fade_ms = 250            # fade duration in milliseconds, 0 = instant
default_unfocused = 0.2  # used when a program has no "unfocused" value

[[program]]
exe = "firefox.exe"      # executable name, case-insensitive
unfocused = 0.1

[[program]]
exe = "eldenring.exe"    # uses default_unfocused
```

To find a program's exe name, open Task Manager, right-click the program, choose **Go to details** and read the name there.

### Changing a game's volume while SoundShift runs

Opening the Volume Mixer takes focus away from the game, so it gets ducked. That's fine:

- If you move the game's slider **while it is ducked**, that value becomes the game's new ducked level. When you go back to the game, it returns to its normal volume. The next time you leave the game, it drops to the level you picked.
- Your normal (focused) volume is read every time you leave the game, so any change you make while playing (for example in the game's own settings) is kept.
- The ducked level you set this way lasts until you reload the config or restart SoundShift. To make it permanent, put it in the config.

## How it works

- **Focus:** `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` tells SoundShift whenever the foreground window changes. It then looks up the window's process and gets its exe name.
- **Volume:** on each focus change it lists the audio sessions of the default output device (`IAudioSessionManager2`) and matches them to the configured programs by exe name. Multi-process apps like browsers are handled as one program.
  - When a program loses focus, its current volume is saved and it fades to its `unfocused` level.
  - When it gets focus again, it fades back to the saved volume.
- **Telling your changes apart from its own:** SoundShift remembers the last volume it set for each audio session. If a session's volume is different from that, you changed it, and SoundShift adopts your value instead of fighting you. New audio sessions (for example a game restarting its audio) are ducked as well, but they never count as your change.
- **Fades:** a 15 ms timer runs only while a fade is in progress, with an ease-out curve.
- **Safety:** volumes are restored on Pause, Exit and Windows shutdown. While something is ducked, its "restore to" level is written to `%APPDATA%\SoundShift\restore.txt`. That lets SoundShift fix the volume after a crash, or when a game was closed while ducked (Windows remembers the lowered volume for the next launch).
- **Tray icon:** drawn in code. It is white on a dark taskbar and black on a light one.

Source layout:

| File | Purpose |
|---|---|
| `src/main.rs` | Hidden window, tray icon and menu, focus hook, message loop |
| `src/audio.rs` | Audio session listing, get/set volume, exe name lookup |
| `src/ducker.rs` | Ducking logic, user-change detection, fades, restore file |
| `src/config.rs` | Loading and creating the TOML config |
| `src/icon.rs` | Generates the tray icon |

See [PLAN.md](PLAN.md) for requirements and future ideas.

## Known limitations

- Games running as administrator: SoundShift may not be able to read their exe name unless it also runs as administrator.
- If you switch the default output device while a game is ducked, the old device's volume is only fixed the next time the game is focused.
- The `.exe` itself still has the default Windows icon in Explorer.
- Windows only for now.
