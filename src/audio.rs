use windows::core::{Interface, Result, GUID, PWSTR};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// Passed with every volume change we make, so our own changes can be told apart
/// from the user's (used later for session event callbacks).
pub const OUR_CONTEXT: GUID = GUID::from_u128(0x5f0d3a52_9c1e_4b7a_8e21_6a4c2b9d7e11);

pub struct Session {
    pub exe: String,
    /// Unique per session instance (stable while the session lives).
    pub id: String,
    vol: ISimpleAudioVolume,
}

impl Session {
    pub fn get(&self) -> f32 {
        unsafe { self.vol.GetMasterVolume().unwrap_or(1.0) }
    }
    pub fn set(&self, v: f32) {
        unsafe {
            let _ = self.vol.SetMasterVolume(v.clamp(0.0, 1.0), &OUR_CONTEXT);
        }
    }
}

pub struct Audio {
    enumerator: IMMDeviceEnumerator,
}

impl Audio {
    pub fn new() -> Result<Self> {
        unsafe {
            Ok(Self {
                enumerator: CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?,
            })
        }
    }

    /// Audio sessions on the default output device whose exe passes `filter`.
    pub fn sessions(&self, filter: impl Fn(&str) -> bool) -> Vec<Session> {
        self.try_sessions(&filter).unwrap_or_default()
    }

    fn try_sessions(&self, filter: &dyn Fn(&str) -> bool) -> Result<Vec<Session>> {
        unsafe {
            let device = self.enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            let mgr: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
            let list = mgr.GetSessionEnumerator()?;
            let mut out = Vec::new();
            for i in 0..list.GetCount()? {
                let Ok(ctl) = list.GetSession(i) else { continue };
                let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else { continue };
                let Ok(pid) = ctl2.GetProcessId() else { continue };
                if pid == 0 {
                    continue;
                }
                let Some(exe) = exe_name(pid) else { continue };
                if !filter(&exe) {
                    continue;
                }
                let id = match ctl2.GetSessionInstanceIdentifier() {
                    Ok(p) => {
                        let s = p.to_string().unwrap_or_default();
                        CoTaskMemFree(Some(p.0 as _));
                        s
                    }
                    Err(_) => continue,
                };
                if let Ok(vol) = ctl.cast::<ISimpleAudioVolume>() {
                    out.push(Session { exe, id, vol });
                }
            }
            Ok(out)
        }
    }
}

/// Lowercase executable file name ("game.exe") of a process.
pub fn exe_name(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        r.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        Some(path.rsplit('\\').next()?.to_lowercase())
    }
}
