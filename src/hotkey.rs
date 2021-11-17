use std::sync::atomic::{AtomicI32, Ordering};
use windows::{
    core::Result,
    Win32::{
        Foundation::HWND,
        UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_CONTROL, MOD_SHIFT},
    },
};

static mut HOT_KEY_ID: AtomicI32 = AtomicI32::new(0);

pub struct HotKey {
    id: i32,
}

impl HotKey {
    // TODO: Allow caller to specify key-combo
    pub fn new() -> Result<Self> {
        let id = unsafe { HOT_KEY_ID.fetch_add(1, Ordering::SeqCst) + 1 };
        unsafe {
            RegisterHotKey(HWND(0), id, MOD_SHIFT | MOD_CONTROL, 0x52 /* R */).ok()?;
        }
        Ok(Self { id })
    }
}

impl Drop for HotKey {
    fn drop(&mut self) {
        unsafe { UnregisterHotKey(HWND(0), self.id).ok().unwrap() }
    }
}
