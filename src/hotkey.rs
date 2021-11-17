use std::sync::atomic::{AtomicI32, Ordering};
use windows::{
    core::Result,
    Win32::{
        Foundation::HWND,
        UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS},
    },
};

static mut HOT_KEY_ID: AtomicI32 = AtomicI32::new(0);

pub struct HotKey {
    id: i32,
}

impl HotKey {
    pub fn new(modifiers: HOT_KEY_MODIFIERS, key: u32) -> Result<Self> {
        let id = unsafe { HOT_KEY_ID.fetch_add(1, Ordering::SeqCst) + 1 };
        unsafe {
            RegisterHotKey(HWND(0), id, modifiers, key).ok()?;
        }
        Ok(Self { id })
    }
}

impl Drop for HotKey {
    fn drop(&mut self) {
        unsafe { UnregisterHotKey(HWND(0), self.id).ok().unwrap() }
    }
}
