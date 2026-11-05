//! Tray icon drawn in code: a monochrome speaker with sound waves (no waves = paused).

use windows::core::w;
use windows::Win32::Foundation::TRUE;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Coverage of the glyph at normalized coords (0..1, y down).
fn inside(x: f32, y: f32, waves: bool) -> bool {
    let dy = (y - 0.5).abs();
    // Speaker body
    if (0.06..=0.26).contains(&x) && dy <= 0.14 {
        return true;
    }
    // Cone
    if (0.26..=0.50).contains(&x) && dy <= 0.14 + (x - 0.26) / 0.24 * 0.28 {
        return true;
    }
    if waves {
        let (dx, r) = (x - 0.50, ((x - 0.50).powi(2) + (y - 0.5).powi(2)).sqrt());
        let in_angle = dx > 0.0 && dy <= dx * 1.3;
        if in_angle && ((r - 0.20).abs() <= 0.055 || (r - 0.38).abs() <= 0.055) {
            return true;
        }
    }
    false
}

fn light_taskbar() -> bool {
    let mut val = 0u32;
    let mut size = 4u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut val as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    r.is_ok() && val != 0
}

/// Builds the icon for the current taskbar theme.
pub fn make(paused: bool) -> HICON {
    unsafe {
        let n = GetSystemMetrics(SM_CXSMICON).max(16);
        let rgb: u32 = if light_taskbar() { 0x000000 } else { 0xFFFFFF };
        const SS: i32 = 4; // supersampling for antialiased edges

        let mut px = vec![0u32; (n * n) as usize];
        for py in 0..n {
            for pxl in 0..n {
                let mut hits = 0;
                for sy in 0..SS {
                    for sx in 0..SS {
                        let x = (pxl as f32 + (sx as f32 + 0.5) / SS as f32) / n as f32;
                        let y = (py as f32 + (sy as f32 + 0.5) / SS as f32) / n as f32;
                        hits += inside(x, y, !paused) as u32;
                    }
                }
                let a = hits * 255 / (SS * SS) as u32;
                px[(py * n + pxl) as usize] = (a << 24) | rgb;
            }
        }

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: n,
                biHeight: -n, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let Ok(color) = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) else {
            return LoadIconW(None, IDI_APPLICATION).unwrap_or_default();
        };
        std::ptr::copy_nonoverlapping(px.as_ptr(), bits as *mut u32, px.len());
        let mask_bytes = vec![0u8; (((n + 15) / 16 * 2) * n) as usize];
        let mask = CreateBitmap(n, n, 1, 1, Some(mask_bytes.as_ptr() as _));

        let info = ICONINFO { fIcon: TRUE, hbmMask: mask, hbmColor: color, ..Default::default() };
        let icon = CreateIconIndirect(&info).unwrap_or_default();
        let _ = DeleteObject(color);
        let _ = DeleteObject(mask);
        icon
    }
}
