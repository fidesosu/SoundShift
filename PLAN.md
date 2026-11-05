# SoundShift – Rework Plan

Lower the volume of configured games/programs when they lose focus, restore it when they regain focus. Lives in the system tray, uses only the Windows audio API (never touches the target process), and stays near-zero CPU.

## Requirements

1. **Tray only** – no console window, no main window. Tray icon with a small menu.
2. **Config file** – list of programs and their "unfocused" volume. Easy to edit, sane defaults.
3. **Focus-based ducking**
   - Program focused → untouched.
   - Program unfocused → fade to its configured unfocused volume.
   - Program refocused → fade back to the volume it had before.
   - **User must not have to fight the app**: changing the volume in the Windows mixer must be respected.
4. **Smooth fades** (optional, nice to have).
5. **Rewrite from scratch** – target Windows 10, should work on 11. Linux is a maybe-later. GUI is a later bonus.
6. **No process modification** – only Windows Core Audio per-app session volume (same thing the Volume Mixer changes).
7. **Lightweight** – event-driven, no busy polling, small binary, minimal deps.

## Tech choices

| Need | How |
|---|---|
| Language | Rust (keep), `windows` crate only. Drop `sysinfo` and `lazy_static`. |
| Hide console | `#![windows_subsystem = "windows"]` |
| Focus detection | `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` – Windows calls us on focus change, **no polling**. |
| Foreground exe name | `GetWindowThreadProcessId` → `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` → `QueryFullProcessImageNameW` |
| Per-app volume | `IMMDeviceEnumerator` → default render device → `IAudioSessionManager2` → enumerate `IAudioSessionControl2` (`GetProcessId`) → `ISimpleAudioVolume::SetMasterVolume` |
| Detect user mixer changes | `IAudioSessionEvents::OnSimpleVolumeChanged` + **event context GUID** (see below) |
| Tray | Raw `Shell_NotifyIconW` + hidden message-only window + `TrackPopupMenu`. No extra crates, and the same message loop drives the WinEvent hook. |
| Config | TOML (`serde` + `toml`) in `%APPDATA%\SoundShift\config.toml`, created with defaults on first run. |
| Fades | `SetTimer` on the hidden window (~15 ms ticks), only running while a fade is active. |

Everything runs on **one thread** with one Win32 message loop (COM initialized as STA). Simple, no locks.

## Config format

```toml
# Volume values are 0.0 – 1.0
fade_ms = 250            # 0 = instant
default_unfocused = 0.2  # used when a program has no explicit value

[[program]]
exe = "firefox.exe"      # case-insensitive match on executable name
unfocused = 0.1

[[program]]
exe = "eldenring.exe"
# unfocused omitted -> default_unfocused
```

Matching by exe name (not PID) handles multi-process apps like browsers: all sessions whose process has that exe name are treated as one program.

Tray menu: `Pause/Resume`, `Reload config`, `Open config`, `Exit`.

## Core logic

State per configured program (in memory):

```
focused_volume:   f32   // what to restore to
unfocused_volume: f32   // what to duck to (from config, may be overridden at runtime)
state:            Focused | Ducked
```

**On focus change** (WinEvent callback):
1. Get foreground exe name.
2. Enumerate audio sessions on the default device (cheap; also picks up new sessions and device changes without extra bookkeeping).
3. For each session of a configured program:
   - If the program just **lost** focus: read current volume → save as `focused_volume` → fade to `unfocused_volume` → `Ducked`.
   - If the program just **gained** focus: fade to `focused_volume` → `Focused`.

**Not fighting the user (the important part):**
Every `SetMasterVolume` call we make passes our own constant GUID as the event context. In `OnSimpleVolumeChanged` we check the context:
- Context == our GUID → our own change, ignore.
- Anything else → **the user changed it** (mixer, in-game slider, etc.):
  - While `Ducked`: cancel any running fade, store the new value as this program's `unfocused_volume` for the session (runtime override). The user tweaks it once and it sticks until restart / config reload.
    *(Later option: write it back to the config file.)*
  - While `Focused`: nothing special – we read the current volume at the next unfocus anyway.

So the user can open the mixer (which unfocuses the game, ducking it), drag the slider to whatever they want, and the app simply adopts it.

**Safety:**
- On `Exit` (and on Ctrl+C / `WM_ENDSESSION`): restore all ducked programs to `focused_volume`.
- Write ducked programs' `focused_volume` to `%APPDATA%\SoundShift\restore.json` while ducked; on startup, if the file exists (previous crash), restore those volumes and delete it.

## Fades

- On start of a fade: remember `from`, `to`, `start_time`.
- Timer tick: `v = from + (to - from) * t` (optionally ease-out), set on all matching sessions.
- When all fades finish → `KillTimer`. Zero CPU when idle.
- A new focus change mid-fade just starts a new fade from the current volume.

## Project layout

```
src/
  main.rs      // init COM, create hidden window, tray, hook, message loop
  tray.rs      // Shell_NotifyIcon + menu
  focus.rs     // SetWinEventHook + foreground exe name
  audio.rs     // session enumeration, get/set volume, session events
  ducker.rs    // per-program state machine + fades (pure logic, testable)
  config.rs    // load/create/reload TOML
```

Keep `ducker.rs` free of Windows types so the core logic can be unit-tested and reused for a Linux backend later.

## Milestones

1. **Skeleton** – tray icon, Exit menu, no console, message loop. Old code removed.
2. **Focus** – log foreground exe on change (debug build: `OutputDebugString` or log file).
3. **Audio** – duck/restore configured programs instantly on focus change. Restore on exit.
4. **Config** – TOML load, defaults file creation, Reload/Open menu items.
5. **User override** – event-context GUID + `OnSimpleVolumeChanged` handling.
6. **Fades** – timer-driven smoothing.
7. **Crash restore** – `restore.json`.
8. *(Later)* Pause toggle, start with Windows (registry `Run` key), write overrides back to config, small GUI (e.g. `egui` settings window opened from tray), Linux backend (PipeWire/PulseAudio sink-input volume).

## Status (v0.2)

Done: milestones 1–7 (tray, focus hook, ducking, TOML config + Reload/Open/Pause, fades, crash restore).

Differences from the plan above:
- Layout is `main.rs` (window/tray/hook), `audio.rs`, `config.rs`, `ducker.rs`. `ducker.rs` still talks to `audio::Session` directly.
- User-override detection does **not** use `IAudioSessionEvents` yet. Instead we remember the last volume we set **per session instance** and compare on each focus change: a tracked session that differs was changed by the user (while ducked → new ducked level; mid-fade → fade stops). New sessions are never mistaken for user changes; while ducked they get ducked too. Our own GUID is already passed, ready for callbacks if needed later.
- The "restore to" level is kept even when the program closes while ducked (Windows remembers the ducked volume for its next launch), and stays in `restore.txt` across SoundShift restarts until the program is focused again.
- Tray icon is drawn in code (`icon.rs`): speaker with waves, without waves when paused; white/black following the taskbar theme.

## Known gotchas

- Fullscreen-exclusive games: alt-tab still fires `EVENT_SYSTEM_FOREGROUND`, should be fine.
- Some games create their audio session late → handled because we re-enumerate on every focus change.
- Elevated (admin) games: `OpenProcess` for the exe name may fail from a non-elevated app; fall back to `PROCESS_QUERY_LIMITED_INFORMATION` (usually allowed). If still failing, note it in the README (run SoundShift as admin).
- Default output device switching mid-duck: the next focus change re-enumerates on the new device; old device's session may stay ducked until refocus. Acceptable for v1.
