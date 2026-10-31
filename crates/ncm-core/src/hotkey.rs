//! 系统级全局快捷键支持（Windows 平台通过 Win32 RegisterHotKey 监听）

#[cfg(windows)]
pub use windows::start_hotkeys;

#[cfg(not(windows))]
pub fn start_hotkeys(_core: &std::sync::Arc<crate::Core>) {}

#[cfg(windows)]
mod windows {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, TranslateMessage, MSG, WM_HOTKEY,
    };

    use crate::Core;

    const HOTKEY_TOGGLE: i32 = 0x8001;
    const HOTKEY_NEXT: i32 = 0x8002;
    const HOTKEY_PREV: i32 = 0x8003;
    const HOTKEY_VOL_UP: i32 = 0x8004;
    const HOTKEY_VOL_DOWN: i32 = 0x8005;

    const MOD_CONTROL: u32 = 0x0002;
    const MOD_NOREPEAT: u32 = 0x4000;

    const VK_LEFT: u32 = 0x25;
    const VK_UP: u32 = 0x26;
    const VK_RIGHT: u32 = 0x27;
    const VK_DOWN: u32 = 0x28;
    const VK_Z: u32 = 0x5A;

    pub fn start_hotkeys(core: &Arc<Core>) {
        static STARTED: AtomicBool = AtomicBool::new(false);
        if STARTED.swap(true, Ordering::SeqCst) {
            return;
        }

        let weak = Arc::downgrade(core);
        let rt = tokio::runtime::Handle::current();

        std::thread::Builder::new()
            .name("ncm-hotkeys".into())
            .spawn(move || unsafe {
                let mods = MOD_CONTROL | MOD_NOREPEAT;
                let null_hwnd: HWND = std::ptr::null_mut();

                // 注册专属全局热键（不受窗口前台/后台/最小化影响）
                let ok_toggle = RegisterHotKey(null_hwnd, HOTKEY_TOGGLE, mods, VK_Z);
                let ok_next = RegisterHotKey(null_hwnd, HOTKEY_NEXT, mods, VK_RIGHT);
                let ok_prev = RegisterHotKey(null_hwnd, HOTKEY_PREV, mods, VK_LEFT);
                let ok_vol_up = RegisterHotKey(null_hwnd, HOTKEY_VOL_UP, mods, VK_UP);
                let ok_vol_down = RegisterHotKey(null_hwnd, HOTKEY_VOL_DOWN, mods, VK_DOWN);

                tracing::info!(
                    "Global hotkeys registered: toggle={}, next={}, prev={}, vol_up={}, vol_down={}",
                    ok_toggle != 0, ok_next != 0, ok_prev != 0, ok_vol_up != 0, ok_vol_down != 0
                );

                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, null_hwnd, 0, 0) > 0 {
                    if msg.message == WM_HOTKEY {
                        let id = msg.wParam as i32;
                        let Some(core) = weak.upgrade() else {
                            break;
                        };
                        match id {
                            HOTKEY_TOGGLE => {
                                core.player.toggle();
                            }
                            HOTKEY_NEXT => {
                                let c = core.clone();
                                rt.spawn(async move { c.next().await; });
                            }
                            HOTKEY_PREV => {
                                let c = core.clone();
                                rt.spawn(async move { c.prev().await; });
                            }
                            HOTKEY_VOL_UP => {
                                core.update_settings(|s| {
                                    s.volume = (s.volume + 0.05).clamp(0.0, 1.0);
                                });
                            }
                            HOTKEY_VOL_DOWN => {
                                core.update_settings(|s| {
                                    s.volume = (s.volume - 0.05).clamp(0.0, 1.0);
                                });
                            }
                            _ => {}
                        }
                    }
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                UnregisterHotKey(null_hwnd, HOTKEY_TOGGLE);
                UnregisterHotKey(null_hwnd, HOTKEY_NEXT);
                UnregisterHotKey(null_hwnd, HOTKEY_PREV);
                UnregisterHotKey(null_hwnd, HOTKEY_VOL_UP);
                UnregisterHotKey(null_hwnd, HOTKEY_VOL_DOWN);
            })
            .ok();
    }
}
