use std::sync::Mutex;
use windows::{
    core::{Interface, Result, HRESULT},
    Win32::Media::Audio::{
        eMultimedia, eRender, IAudioSessionControl, IAudioSessionControl2, IAudioSessionEnumerator,
        IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
    },
    Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
};

// Global variable to store the previous volume
lazy_static::lazy_static! {
    static ref PREVIOUS_VOLUME: Mutex<Option<f32>> = Mutex::new(None);
}

fn hresult_to_result(hr: HRESULT) -> Result<()> {
    if hr.is_ok() {
        Ok(())
    } else {
        Err(hr.into())
    }
}

fn get_audio_volume_interface(process_id: u32) -> Result<ISimpleAudioVolume> {
    unsafe {
        // Initialize COM library
        hresult_to_result(CoInitializeEx(None, COINIT_MULTITHREADED))?;

        // Get the IMMDeviceEnumerator
        let device_enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

        // Get the default audio endpoint
        let device = device_enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;

        // Get the IAudioSessionManager2
        let session_manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;

        // Get the IAudioSessionEnumerator
        let session_enumerator: IAudioSessionEnumerator = session_manager.GetSessionEnumerator()?;

        // Get the number of audio sessions
        let session_count = session_enumerator.GetCount()?;

        // Iterate through the audio sessions
        for i in 0..session_count {
            let session_control: IAudioSessionControl = session_enumerator.GetSession(i)?;

            // Query for IAudioSessionControl2 to get the process ID
            let session_control2: IAudioSessionControl2 = session_control.cast()?;

            // Get the process ID of the session
            let session_process_id = session_control2.GetProcessId()?;

            if session_process_id == process_id {
                // Get the ISimpleAudioVolume
                let audio_volume: ISimpleAudioVolume = session_control.cast()?;
                return Ok(audio_volume);
            }
        }
    }
    Err(HRESULT::from_win32(0).into())
}

pub fn set_app_volume(process_id: u32, volume: f32) -> Result<()> {
    let audio_volume = get_audio_volume_interface(process_id)?;

    unsafe {
        // Store the current volume
        let current_volume = audio_volume.GetMasterVolume()?;
        *PREVIOUS_VOLUME.lock().unwrap() = Some(current_volume);

        // Set the volume
        audio_volume.SetMasterVolume(volume, std::ptr::null_mut())?;
    }
    Ok(())
}

pub fn get_app_volume(process_id: u32) -> Result<f32> {
    let audio_volume = get_audio_volume_interface(process_id)?;

    unsafe {
        // Get the current volume
        let current_volume = audio_volume.GetMasterVolume()?;
        Ok(current_volume)
    }
}