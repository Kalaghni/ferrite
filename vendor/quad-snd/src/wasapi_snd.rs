// https://github.com/floooh/sokol/blob/master/sokol_audio.h
// https://github.com/norse-rs/audir/blob/master/audir/src/wasapi/mod.rs

use crate::PlaySoundParams;

pub use crate::mixer::Playback;

use winapi::shared::guiddef::{CLSID, IID};
use winapi::shared::ksmedia;
use winapi::shared::minwindef::*;
use winapi::shared::mmreg::*;
use winapi::um::audioclient::*;
use winapi::um::audiosessiontypes::*;
use winapi::um::combaseapi::*;
use winapi::um::mmdeviceapi::*;
use winapi::um::objbase::*;
use winapi::um::synchapi::*;
use winapi::um::winbase::*;

use std::sync::mpsc;

// thanks sokol_audio!
// https://github.com/floooh/sokol/blob/master/sokol_audio.h#L559
static IID_IAudioClient: IID = IID {
    Data1: 0x1cb9ad4c,
    Data2: 0xdbfa,
    Data3: 0x4c32,
    Data4: [0xb1, 0x78, 0xc2, 0xf5, 0x68, 0xa7, 0x03, 0xb2],
};
static IID_IMMDeviceEnumerator: IID = IID {
    Data1: 0xa95664d2,
    Data2: 0x9614,
    Data3: 0x4f35,
    Data4: [0xa7, 0x46, 0xde, 0x8d, 0xb6, 0x36, 0x17, 0xe6],
};
static CLSID_IMMDeviceEnumerator: CLSID = CLSID {
    Data1: 0xbcde0395,
    Data2: 0xe52f,
    Data3: 0x467c,
    Data4: [0x8e, 0x3d, 0xc4, 0x57, 0x92, 0x91, 0x69, 0x2e],
};
static IID_IAudioRenderClient: IID = IID {
    Data1: 0xf294acfc,
    Data2: 0x3146,
    Data3: 0x4483,
    Data4: [0xa7, 0xbf, 0xad, 0xdc, 0xa7, 0xc2, 0x60, 0xe2],
};
static IID_Devinterface_Audio_Render: IID = IID {
    Data1: 0xe6327cad,
    Data2: 0xdcec,
    Data3: 0x4949,
    Data4: [0xae, 0x8a, 0x99, 0x1e, 0x97, 0x6a, 0x79, 0xd2],
};
static IID_IActivateAudioInterface_Completion_Handler: IID = IID {
    Data1: 0x94ea2b94,
    Data2: 0xe9cc,
    Data3: 0x49e0,
    Data4: [0xc0, 0xff, 0xee, 0x64, 0xca, 0x8f, 0x5b, 0x90],
};

mod consts {
    pub const CHANNELS: u32 = 2;
    pub const SAMPLE_RATE: u32 = 44100;
    pub const BUFFER_FRAMES: u32 = 4096;
}

// ---- FERRITE: output device selection ----
// The stream can be moved to another device while sounds are playing, and
// it is reopened (on the system default if need be) when a device goes away.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use winapi::shared::wtypes::VT_LPWSTR;
use winapi::um::coml2api::STGM_READ;
use winapi::um::functiondiscoverykeys_devpkey::PKEY_Device_FriendlyName;
use winapi::um::propidl::PROPVARIANT;
use winapi::um::propsys::IPropertyStore;

static WANTED: Mutex<Option<String>> = Mutex::new(None);
static GENERATION: AtomicU32 = AtomicU32::new(0);

/// Play through the device with this id (from `output_devices`); `None`
/// follows the system default.
pub fn set_output_device(id: Option<String>) {
    let mut w = WANTED.lock().unwrap();
    if *w != id {
        *w = id;
        GENERATION.fetch_add(1, Ordering::SeqCst);
    }
}

/// Active output devices as (id, name).
pub fn output_devices() -> Vec<(String, String)> {
    // On a thread of its own so COM on the caller's thread is left alone.
    std::thread::spawn(|| unsafe { list_devices() }).join().unwrap_or_default()
}

unsafe fn from_wide(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0;
    while *p.add(n) != 0 {
        n += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
}

unsafe fn enumerator() -> *mut IMMDeviceEnumerator {
    CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED);
    let mut e: *mut IMMDeviceEnumerator = std::ptr::null_mut();
    let hr = CoCreateInstance(&CLSID_IMMDeviceEnumerator, std::ptr::null_mut(), CLSCTX_ALL, &IID_IMMDeviceEnumerator, &mut e as *mut _ as _);
    if hr < 0 {
        return std::ptr::null_mut();
    }
    e
}

unsafe fn list_devices() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let e = enumerator();
    if e.is_null() {
        return out;
    }
    let mut coll: *mut IMMDeviceCollection = std::ptr::null_mut();
    if (*e).EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE, &mut coll) >= 0 && !coll.is_null() {
        let mut n = 0;
        (*coll).GetCount(&mut n);
        for i in 0..n {
            let mut d: *mut IMMDevice = std::ptr::null_mut();
            if (*coll).Item(i, &mut d) < 0 || d.is_null() {
                continue;
            }
            let mut idp: *mut u16 = std::ptr::null_mut();
            if (*d).GetId(&mut idp) >= 0 && !idp.is_null() {
                let id = from_wide(idp);
                CoTaskMemFree(idp as _);
                let mut name = String::new();
                let mut store: *mut IPropertyStore = std::ptr::null_mut();
                if (*d).OpenPropertyStore(STGM_READ, &mut store) >= 0 && !store.is_null() {
                    let mut pv: PROPVARIANT = std::mem::zeroed();
                    if (*store).GetValue(&PKEY_Device_FriendlyName, &mut pv) >= 0 {
                        if pv.vt == VT_LPWSTR as u16 {
                            name = from_wide(*pv.data.pwszVal());
                        }
                        PropVariantClear(&mut pv);
                    }
                    (*store).Release();
                }
                if name.is_empty() {
                    name = "Unknown device".to_string();
                }
                out.push((id, name));
            }
            (*d).Release();
        }
        (*coll).Release();
    }
    (*e).Release();
    out
}

struct Output {
    device: *mut IMMDevice,
    client: *mut IAudioClient,
    render: *mut IAudioRenderClient,
    frames: u32,
}

impl Output {
    unsafe fn close(self) {
        (*self.client).Stop();
        if !self.render.is_null() {
            (*self.render).Release();
        }
        (*self.client).Release();
        (*self.device).Release();
    }
}

/// Open a shared-mode stream on the wanted device, or on the system
/// default when there is no wish or the wanted device is not usable.
unsafe fn open_output(e: *mut IMMDeviceEnumerator, wanted: &Option<String>, event: winapi::um::winnt::HANDLE) -> Option<Output> {
    let mut device: *mut IMMDevice = std::ptr::null_mut();
    if let Some(id) = wanted {
        let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
        if (*e).GetDevice(wide.as_ptr(), &mut device) >= 0 && !device.is_null() {
            let mut state = 0;
            (*device).GetState(&mut state);
            if state & DEVICE_STATE_ACTIVE == 0 {
                (*device).Release();
                device = std::ptr::null_mut();
            }
        } else {
            device = std::ptr::null_mut();
        }
    }
    if device.is_null() && ((*e).GetDefaultAudioEndpoint(eRender, eConsole, &mut device) < 0 || device.is_null()) {
        return None;
    }
    let mut client: *mut IAudioClient = std::ptr::null_mut();
    if (*device).Activate(&IID_IAudioClient, CLSCTX_ALL, std::ptr::null_mut(), &mut client as *mut _ as _) < 0 || client.is_null() {
        (*device).Release();
        return None;
    }
    let format = WAVEFORMATEX {
        nChannels: consts::CHANNELS as _,
        nSamplesPerSec: consts::SAMPLE_RATE as _,
        wFormatTag: WAVE_FORMAT_EXTENSIBLE,
        wBitsPerSample: 32,
        nBlockAlign: consts::CHANNELS as u16 * 4,
        nAvgBytesPerSec: consts::CHANNELS as u32 * consts::SAMPLE_RATE as u32 * 4,
        cbSize: (std::mem::size_of::<WAVEFORMATEXTENSIBLE>() - std::mem::size_of::<WAVEFORMATEX>()) as _,
    };
    const FRONT_LEFT: u32 = 0b0001;
    const FRONT_RIGHT: u32 = 0b0010;
    let format_extensible = WAVEFORMATEXTENSIBLE {
        Format: format,
        Samples: 4 * 8,
        dwChannelMask: FRONT_LEFT | FRONT_RIGHT,
        SubFormat: ksmedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
    };
    // https://docs.microsoft.com/en-us/windows/win32/coreaudio/audclnt-streamflags-xxx-constants
    const AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM: u32 = 0x80000000;
    const AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY: u32 = 0x08000000;
    let dur = consts::BUFFER_FRAMES as f64 / (consts::SAMPLE_RATE as f64 * 1.0 / 10000000.0);
    let mut out = Output { device, client, render: std::ptr::null_mut(), frames: 0 };
    let ok = (*client).Initialize(
        AUDCLNT_SHAREMODE_SHARED,
        AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
        dur as _,
        0,
        &format_extensible as *const _ as _,
        std::ptr::null(),
    ) >= 0
        && (*client).GetBufferSize(&mut out.frames) >= 0
        && (*client).GetService(&IID_IAudioRenderClient, &mut out.render as *mut _ as _) >= 0
        && !out.render.is_null()
        && (*client).SetEventHandle(event) >= 0
        && (*client).Start() >= 0;
    if !ok {
        out.close();
        return None;
    }
    Some(out)
}

unsafe fn audio_thread(mut mixer: crate::mixer::Mixer) {
    let buffer_end_event = CreateEventA(std::ptr::null_mut(), FALSE, FALSE, std::ptr::null());
    assert!(buffer_end_event.is_null() == false);
    let device_enumerator = enumerator();
    assert!(!device_enumerator.is_null(), "CoCreatInstance failed");

    loop {
        let generation = GENERATION.load(Ordering::SeqCst);
        let wanted = WANTED.lock().unwrap().clone();
        let Some(out) = open_output(device_enumerator, &wanted, buffer_end_event) else {
            // Nothing to play through right now: look again shortly.
            std::thread::sleep(std::time::Duration::from_millis(500));
            continue;
        };
        let mut quiet = 0;
        loop {
            if GENERATION.load(Ordering::SeqCst) != generation {
                break; // another device was chosen
            }
            if WaitForSingleObject(buffer_end_event, 500) != WAIT_OBJECT_0 {
                // A device that stops asking for audio has gone away.
                quiet += 1;
                if quiet >= 3 {
                    break;
                }
                continue;
            }
            quiet = 0;
            let mut padding = 0;
            if (*out.client).GetCurrentPadding(&mut padding) < 0 {
                break; // invalidated (unplugged, format changed): reopen
            }
            let num_frames = out.frames.saturating_sub(padding);
            if num_frames == 0 {
                continue;
            }
            let mut wasapi_buffer: *mut u8 = std::ptr::null_mut();
            if (*out.render).GetBuffer(num_frames, &mut wasapi_buffer) < 0 || wasapi_buffer.is_null() {
                break;
            }
            let buffer = std::slice::from_raw_parts_mut(wasapi_buffer as *mut f32, num_frames as usize * consts::CHANNELS as usize);
            mixer.fill_audio_buffer(buffer, num_frames as _);
            (*out.render).ReleaseBuffer(num_frames, 0);
        }
        out.close();
    }
}

pub struct AudioContext {
    pub(crate) mixer_ctrl: crate::mixer::MixerControl,
}

impl AudioContext {
    pub fn new() -> AudioContext {
        use crate::mixer::Mixer;

        let (mixer_builder, mixer_ctrl) = Mixer::new();
        std::thread::spawn(move || unsafe {
            audio_thread(mixer_builder.build());
        });

        AudioContext { mixer_ctrl }
    }
}

pub struct Sound {
    sound_id: u32,
}

impl Sound {
    pub fn load(ctx: &AudioContext, data: &[u8]) -> Sound {
        let sound_id = ctx.mixer_ctrl.load(data);

        Sound { sound_id }
    }

    pub fn play(&self, ctx: &AudioContext, params: PlaySoundParams) -> Playback {
        ctx.mixer_ctrl.play(self.sound_id, params)
    }

    pub fn stop(&self, ctx: &AudioContext) {
        ctx.mixer_ctrl.stop_all(self.sound_id);
    }

    pub fn set_volume(&self, ctx: &AudioContext, volume: f32) {
        ctx.mixer_ctrl.set_volume_all(self.sound_id, volume);
    }

    pub fn delete(&self, ctx: &AudioContext) {
        ctx.mixer_ctrl.delete(self.sound_id);
    }
}
