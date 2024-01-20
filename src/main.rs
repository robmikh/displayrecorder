mod args;
mod capture;
mod d3d;
mod displays;
mod hotkey;
mod media;
mod resolution;
mod video;

use std::{path::Path, time::Duration};

use args::Args;
use clap::Parser;
use hotkey::HotKey;
use windows::{
    core::{Result, RuntimeName, HSTRING},
    Foundation::Metadata::ApiInformation,
    Graphics::{
        Capture::{GraphicsCaptureItem, GraphicsCaptureSession},
        SizeInt32,
    },
    Storage::{
        CreationCollisionOption, FileAccessMode, StorageFolder, Streams::IRandomAccessStream,
    },
    Win32::{
        Foundation::{HWND, MAX_PATH},
        Graphics::Direct3D11::ID3D11Device,
        Media::MediaFoundation::{MFStartup, MFSTARTUP_FULL},
        Storage::FileSystem::GetFullPathNameW,
        System::{
            Diagnostics::Debug::{DebugBreak, IsDebuggerPresent},
            Threading::GetCurrentProcessId,
            WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
        },
        UI::{
            Input::KeyboardAndMouse::{MOD_CONTROL, MOD_SHIFT},
            WindowsAndMessaging::{DispatchMessageW, GetMessageW, MSG, WM_HOTKEY},
        },
    },
};

use crate::{
    capture::create_capture_item_for_monitor,
    d3d::create_d3d_device,
    displays::get_display_handle_from_index,
    media::MF_VERSION,
    resolution::Resolution,
    video::mf::{encoder_device::VideoEncoderDevice, encoding_session::VideoEncodingSession},
};

#[allow(clippy::too_many_arguments)]
fn run(
    display_index: usize,
    output_path: &str,
    bit_rate: u32,
    frame_rate: u32,
    resolution: Resolution,
    encoder_index: usize,
    verbose: bool,
    wait_for_debugger: bool,
    console_mode: bool,
) -> Result<()> {
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED)?;
    }
    unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL)? }

    if wait_for_debugger {
        let pid = unsafe { GetCurrentProcessId() };
        println!("Waiting for a debugger to attach (PID: {})...", pid);
        loop {
            if unsafe { IsDebuggerPresent().into() } {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        unsafe {
            DebugBreak();
        }
    }

    // Check to make sure Windows.Graphics.Capture is available
    if !required_capture_features_supported()? {
        exit_with_error("The required screen capture features are not supported on this device for this release of Windows!\nPlease update your operating system (minimum: Windows 10 Version 1903, Build 18362).");
    }

    if verbose {
        println!(
            "Using index \"{}\" and path \"{}\".",
            display_index, output_path
        );
    }

    // Get the display handle using the provided index
    let display_handle = get_display_handle_from_index(display_index)
        .expect("The provided display index was out of bounds!");
    let item = create_capture_item_for_monitor(display_handle)?;

    // Resolve encoding settings
    let resolution = if let Some(resolution) = resolution.get_size() {
        resolution
    } else {
        item.Size()?
    };
    let bit_rate = bit_rate * 1000000;
    let encoder_devices = VideoEncoderDevice::enumerate()?;
    if encoder_devices.is_empty() {
        exit_with_error("No hardware H264 encoders found!");
    }
    if verbose {
        println!("Encoders ({}):", encoder_devices.len());
        for encoder_device in &encoder_devices {
            println!("  {}", encoder_device.display_name());
        }
    }
    let encoder_device = if let Some(encoder_device) = encoder_devices.get(encoder_index) {
        encoder_device
    } else {
        exit_with_error("Encoder index is out of bounds!");
    };
    if verbose {
        println!("Using: {}", encoder_device.display_name());
    }

    // Create our file
    let path = unsafe {
        let mut new_path = vec![0u16; MAX_PATH as usize];
        let length = GetFullPathNameW(&HSTRING::from(output_path), Some(&mut new_path), None);
        new_path.resize(length as usize, 0);
        String::from_utf16(&new_path).unwrap()
    };
    let path = Path::new(&path);
    let parent_folder_path = path.parent().unwrap();
    let parent_folder = StorageFolder::GetFolderFromPathAsync(&HSTRING::from(
        parent_folder_path.as_os_str().to_str().unwrap(),
    ))?
    .get()?;
    let file_name = path.file_name().unwrap();
    let file = parent_folder
        .CreateFileAsync(
            &HSTRING::from(file_name.to_str().unwrap()),
            CreationCollisionOption::ReplaceExisting,
        )?
        .get()?;

    // Start the recording
    {
        let stream = file.OpenAsync(FileAccessMode::ReadWrite)?.get()?;
        let d3d_device = create_d3d_device()?;
        let mut session = create_encoding_session(
            d3d_device,
            item,
            encoder_device,
            resolution,
            bit_rate,
            frame_rate,
            stream,
        )?;
        if !console_mode {
            let mut is_recording = false;
            pump_messages(|| -> Result<bool> {
                Ok(if !is_recording {
                    is_recording = true;
                    println!("Starting recording...");
                    session.start()?;
                    false
                } else {
                    true
                })
            })?;
            println!("Stopping recording...");
        } else {
            session.start()?;
            pause();
        }
        session.stop()?;
    }

    Ok(())
}

fn main() {
    // Handle /?
    let args: Vec<_> = std::env::args().collect();
    if args.contains(&"/?".to_owned()) || args.contains(&"-?".to_owned()) {
        Args::parse_from(["displayrecorder.exe", "--help"]);
        std::process::exit(0);
    }

    let args = Args::parse();

    if let Some(command) = args.command {
        match command {
            args::Commands::EnumEncoders => enum_encoders().unwrap(),
        }
        return;
    }

    let monitor_index: usize = args.display;
    let output_path = args.output_file.as_str();
    let verbose = args.verbose;
    let wait_for_debugger = args.wait_for_debugger;
    let console_mode = args.console_mode;
    let bit_rate: u32 = args.bit_rate;
    let frame_rate: u32 = args.frame_rate;
    let resolution: Resolution = args.resolution;
    let encoder_index: usize = args.encoder;

    // Validate some of the params
    if !validate_path(output_path) {
        exit_with_error("Invalid path specified!");
    }

    let result = run(
        monitor_index,
        output_path,
        bit_rate,
        frame_rate,
        resolution,
        encoder_index,
        verbose | wait_for_debugger,
        wait_for_debugger,
        console_mode,
    );

    // We do this for nicer HRESULT printing when errors occur.
    if let Err(error) = result {
        error.code().unwrap();
    }
}

fn pause() {
    println!("Press ENTER to stop recording...");
    std::io::Read::read(&mut std::io::stdin(), &mut [0]).unwrap();
}

fn enum_encoders() -> Result<()> {
    let encoder_devices = VideoEncoderDevice::enumerate()?;
    if encoder_devices.is_empty() {
        exit_with_error("No hardware H264 encoders found!");
    }
    println!("Encoders ({}):", encoder_devices.len());
    for (i, encoder_device) in encoder_devices.iter().enumerate() {
        println!("  {} - {}", i, encoder_device.display_name());
    }
    Ok(())
}

fn create_encoding_session(
    d3d_device: ID3D11Device,
    item: GraphicsCaptureItem,
    encoder_device: &VideoEncoderDevice,
    resolution: SizeInt32,
    bit_rate: u32,
    frame_rate: u32,
    stream: IRandomAccessStream,
) -> Result<VideoEncodingSession> {
    let result = VideoEncodingSession::new(
        d3d_device,
        item,
        encoder_device,
        resolution,
        bit_rate,
        frame_rate,
        stream,
    );
    if result.is_err() {
        println!("Error during encoder setup, try another set of encoding settings.");
    }
    result
}

fn validate_path<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    let mut valid = true;
    if let Some(extension) = path.extension() {
        if extension != "mp4" {
            valid = false;
        }
    } else {
        valid = false;
    }
    valid
}

fn exit_with_error(message: &str) -> ! {
    println!("{}", message);
    std::process::exit(1);
}

fn win32_programmatic_capture_supported() -> Result<bool> {
    ApiInformation::IsApiContractPresentByMajor(
        &HSTRING::from("Windows.Foundation.UniversalApiContract"),
        8,
    )
}

fn required_capture_features_supported() -> Result<bool> {
    let result = ApiInformation::IsTypePresent(&HSTRING::from(GraphicsCaptureSession::NAME))? && // Windows.Graphics.Capture is present
    GraphicsCaptureSession::IsSupported()? && // The CaptureService is available
    win32_programmatic_capture_supported()?;
    Ok(result)
}

fn pump_messages<F: FnMut() -> Result<bool>>(mut hot_key_callback: F) -> Result<()> {
    let _hot_key = HotKey::new(MOD_SHIFT | MOD_CONTROL, 0x52 /* R */)?;
    println!("Press SHIFT+CTRL+R to start/stop the recording...");
    unsafe {
        let mut message = MSG::default();
        while GetMessageW(&mut message, HWND(0), 0, 0).into() {
            if message.message == WM_HOTKEY && hot_key_callback()? {
                break;
            }
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::validate_path;

    #[test]
    fn path_parsing_test() {
        assert!(validate_path("something.mp4"));
        assert!(validate_path("somedir/something.mp4"));
        assert!(validate_path("somedir\\something.mp4"));
        assert!(validate_path("../something.mp4"));

        assert!(!validate_path("."));
        assert!(!validate_path("*"));
        assert!(!validate_path("something"));
        assert!(!validate_path(".mp4"));
        assert!(!validate_path("mp4"));
        assert!(!validate_path("something.avi"));
    }
}
