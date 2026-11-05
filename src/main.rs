#![windows_subsystem = "windows"]

mod audio;
mod config;
mod ducker;
mod icon;

use ducker::Ducker;
use std::cell::{Cell, RefCell};
use windows::core::{w, Result, HSTRING, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_TRAY: u32 = WM_APP + 1;
const TIMER_ID: usize = 1;
const FADE_TICK_MS: u32 = 15;

const ID_PAUSE: usize = 1;
const ID_RELOAD: usize = 2;
const ID_OPEN: usize = 3;
const ID_EXIT: usize = 4;

thread_local! {
    static DUCKER: RefCell<Option<Ducker>> = const { RefCell::new(None) };
    static MAIN_HWND: Cell<HWND> = Cell::new(HWND::default());
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
    static ICON: Cell<HICON> = Cell::new(HICON::default());
}

/// Run `f` on the ducker; `None` if unavailable (e.g. re-entrant call).
fn with_ducker<R>(f: impl FnOnce(&mut Ducker) -> R) -> Option<R> {
    DUCKER.with(|d| d.try_borrow_mut().ok()?.as_mut().map(f))
}

fn ensure_timer(needed: Option<bool>) {
    if needed == Some(true) {
        unsafe { SetTimer(MAIN_HWND.get(), TIMER_ID, FADE_TICK_MS, None) };
    }
}

fn error_box(msg: &str) {
    unsafe { MessageBoxW(None, &HSTRING::from(msg), w!("SoundShift"), MB_ICONERROR | MB_OK) };
}

fn main() -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;

        let cfg = config::load().unwrap_or_else(|e| {
            error_box(&e);
            config::Config::default()
        });
        let audio = audio::Audio::new()?;

        let hinst: HINSTANCE = GetModuleHandleW(None)?.into();
        let class = w!("SoundShiftWnd");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinst,
            lpszClassName: class,
            ..Default::default()
        });
        // Hidden top-level window (never shown): receives tray, menu and timer messages.
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!("SoundShift"),
            WS_OVERLAPPED,
            0, 0, 0, 0,
            None,
            None,
            hinst,
            None,
        )?;
        MAIN_HWND.set(hwnd);
        TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarCreated")));

        DUCKER.with(|d| *d.borrow_mut() = Some(Ducker::new(audio, cfg)));
        tray(hwnd, NIM_ADD);

        // Event-driven focus tracking: Windows calls us, no polling.
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(on_foreground),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        handle_foreground(GetForegroundWindow());

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = UnhookWinEvent(hook);
    }
    Ok(())
}

fn handle_foreground(hwnd: HWND) {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let exe = audio::exe_name(pid).unwrap_or_default();
    ensure_timer(with_ducker(|d| d.on_focus(exe)));
}

unsafe extern "system" fn on_foreground(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    handle_foreground(hwnd);
}

/// Add/modify/delete the tray icon. Add and modify redraw the icon (pause state, taskbar theme).
unsafe fn tray(hwnd: HWND, action: NOTIFY_ICON_MESSAGE) {
    let paused = with_ducker(|d| d.paused).unwrap_or(false);
    if action != NIM_DELETE {
        let old = ICON.replace(icon::make(paused));
        if !old.is_invalid() {
            let _ = DestroyIcon(old);
        }
    }
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        hIcon: ICON.get(),
        ..Default::default()
    };
    let tip: Vec<u16> =
        if paused { "SoundShift (paused)" } else { "SoundShift" }.encode_utf16().collect();
    nid.szTip[..tip.len()].copy_from_slice(&tip);
    let _ = Shell_NotifyIconW(action, &nid);
}

unsafe fn show_menu(hwnd: HWND) {
    let Ok(menu) = CreatePopupMenu() else { return };
    let paused = with_ducker(|d| d.paused).unwrap_or(false);
    let check = if paused { MF_CHECKED } else { MF_UNCHECKED };
    let _ = AppendMenuW(menu, MF_STRING | check, ID_PAUSE, w!("Pause"));
    let _ = AppendMenuW(menu, MF_STRING, ID_RELOAD, w!("Reload config"));
    let _ = AppendMenuW(menu, MF_STRING, ID_OPEN, w!("Open config"));
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, ID_EXIT, w!("Exit"));
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let _ = SetForegroundWindow(hwnd); // required so the menu closes when clicking elsewhere
    let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, None);
    let _ = DestroyMenu(menu);
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            let ev = (lparam.0 & 0xffff) as u32;
            if ev == WM_RBUTTONUP || ev == WM_LBUTTONUP {
                show_menu(hwnd);
            }
        }
        WM_COMMAND => match wparam.0 & 0xffff {
            ID_PAUSE => {
                ensure_timer(with_ducker(|d| d.set_paused(!d.paused)));
                tray(hwnd, NIM_MODIFY);
            }
            ID_RELOAD => match config::load() {
                Ok(cfg) => ensure_timer(with_ducker(|d| d.apply_config(cfg))),
                Err(e) => error_box(&e),
            },
            ID_OPEN => {
                let _ = std::process::Command::new("explorer").arg(config::path()).spawn();
            }
            ID_EXIT => {
                let _ = DestroyWindow(hwnd);
            }
            _ => {}
        },
        WM_TIMER => {
            if with_ducker(|d| d.tick()) == Some(false) {
                let _ = KillTimer(hwnd, TIMER_ID);
            }
        }
        WM_SETTINGCHANGE => tray(hwnd, NIM_MODIFY), // taskbar light/dark theme may have changed
        WM_ENDSESSION => {
            with_ducker(|d| d.restore_all());
        }
        WM_DESTROY => {
            with_ducker(|d| d.restore_all());
            tray(hwnd, NIM_DELETE);
            PostQuitMessage(0);
        }
        m if m == TASKBAR_CREATED.get() && m != 0 => tray(hwnd, NIM_ADD), // explorer restarted
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    LRESULT(0)
}
