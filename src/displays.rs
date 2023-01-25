use windows::Win32::{
    Foundation::{BOOL, LPARAM, RECT},
    Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR},
};

pub fn get_display_handle_from_index(index: usize) -> Option<HMONITOR> {
    let displays = enumerate_displays();
    if let Some(handle) = displays.get(index) {
        Some(*handle)
    } else {
        None
    }
}

fn enumerate_displays() -> Box<Vec<HMONITOR>> {
    unsafe {
        let displays = Box::into_raw(Box::new(Vec::<HMONITOR>::new()));
        EnumDisplayMonitors(HDC(0), None, Some(enum_monitor), LPARAM(displays as isize));
        Box::from_raw(displays)
    }
}

extern "system" fn enum_monitor(monitor: HMONITOR, _: HDC, _: *mut RECT, state: LPARAM) -> BOOL {
    unsafe {
        let state = Box::leak(Box::from_raw(state.0 as *mut Vec<HMONITOR>));
        state.push(monitor);
    }
    true.into()
}
