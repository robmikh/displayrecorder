mod capture;
mod d3d;
mod displays;
mod media;
mod video;

use std::{
    io::{stdin, stdout, Read, Write},
    path::Path,
    time::Duration,
};

use clap::{App, Arg};
use windows::{
    runtime::Result,
    Storage::{CreationCollisionOption, FileAccessMode, StorageFolder},
    Win32::{
        Foundation::{MAX_PATH, PWSTR},
        Media::MediaFoundation::{MFStartup, MFSTARTUP_FULL},
        Storage::FileSystem::GetFullPathNameW,
        System::{
            Diagnostics::Debug::{DebugBreak, IsDebuggerPresent},
            Threading::GetCurrentProcessId,
            WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
        },
    },
};

use crate::{
    capture::create_capture_item_for_monitor,
    d3d::create_d3d_device,
    displays::get_display_handle_from_index,
    media::MF_VERSION,
    video::{encoder_device::VideoEncoderDevice, encoding_session::VideoEncodingSession},
};

fn run(
    display_index: usize,
    output_path: &str,
    verbose: bool,
    wait_for_debugger: bool,
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

    // TODO: Make these encoding settings configurable
    let resolution = item.Size()?;
    let bit_rate = 18000000;
    let frame_rate = 60;
    let encoder_devices = VideoEncoderDevice::enumerate()?;
    if verbose {
        println!("Encoders ({}):", encoder_devices.len());
        for encoder_device in &encoder_devices {
            println!("  {}", encoder_device.display_name());
        }
    }
    let encoder_device = &encoder_devices[0];
    if verbose {
        println!("Using: {}", encoder_device.display_name());
    }

    let path = unsafe {
        let mut output_path: Vec<u16> = output_path.encode_utf16().collect();
        output_path.push(0);
        let mut new_path = vec![0u16; MAX_PATH as usize];
        let length = GetFullPathNameW(
            PWSTR(output_path.as_mut_ptr()),
            new_path.len() as u32,
            PWSTR(new_path.as_mut_ptr()),
            std::ptr::null_mut(),
        );
        new_path.resize(length as usize, 0);
        String::from_utf16(&new_path).unwrap()
    };
    let path = Path::new(&path);
    let parent_folder_path = path.parent().unwrap();
    let parent_folder =
        StorageFolder::GetFolderFromPathAsync(parent_folder_path.as_os_str().to_str().unwrap())?
            .get()?;
    let file_name = path.file_name().unwrap();
    let file = parent_folder
        .CreateFileAsync(
            file_name.to_str().unwrap(),
            CreationCollisionOption::ReplaceExisting,
        )?
        .get()?;

    {
        let stream = file.OpenAsync(FileAccessMode::ReadWrite)?.get()?;
        let d3d_device = create_d3d_device()?;
        let mut session = VideoEncodingSession::new(
            d3d_device,
            item,
            encoder_device,
            resolution,
            bit_rate,
            frame_rate,
            stream,
        )?;
        session.start()?;
        pause();
        session.stop()?;
    }

    Ok(())
}

fn main() {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::with_name("display")
                .short("d")
                .long("display")
                .value_name("display index")
                .help("The index of the display you'd like to record.")
                .takes_value(true)
                .default_value("0")
                .required(false),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .help("Enables verbose (debug) output")
                .required(false),
        )
        .arg(
            Arg::with_name("waitForDebugger")
                .long("waitForDebugger")
                .help("The program will wait for a debugger to attach before starting.")
                .required(false),
        )
        .arg(
            Arg::with_name("OUTPUT FILE")
                .help("The output file that will contain the recording.")
                .default_value("recording.mp4")
                .required(false),
        )
        .get_matches();

    let monitor_index: usize = matches.value_of("display").unwrap().parse().unwrap();
    let output_path = matches.value_of("OUTPUT FILE").unwrap();
    let verbose = matches.is_present("verbose");
    let wait_for_debugger = matches.is_present("waitForDebugger");

    let result = run(
        monitor_index,
        output_path,
        verbose | wait_for_debugger,
        wait_for_debugger,
    );

    // We do this for nicer HRESULT printing when errors occur.
    if let Err(error) = result {
        error.code().unwrap();
    }
}

fn pause() {
    let mut stdout = stdout();
    stdout.write(b"Press ENTER to stop recording...").unwrap();
    stdout.flush().unwrap();
    stdin().read(&mut [0]).unwrap();
}
