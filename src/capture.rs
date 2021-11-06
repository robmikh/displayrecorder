use windows::{
    runtime::Result,
    Graphics::Capture::GraphicsCaptureItem,
    Win32::{Graphics::Gdi::HMONITOR, System::WinRT::IGraphicsCaptureItemInterop},
};

pub fn create_capture_item_for_monitor(monitor_handle: HMONITOR) -> Result<GraphicsCaptureItem> {
    let interop = windows::runtime::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
    unsafe { interop.CreateForMonitor(monitor_handle) }
}
