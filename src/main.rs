mod capture;
mod displays;
mod encoder;

use clap::{App, Arg};
use windows::{
    runtime::Result,
    Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
};

use crate::{
    capture::create_capture_item_for_monitor, displays::get_display_handle_from_index,
    encoder::VideoEncoderDevice,
};

fn run(display_index: usize, output_path: &str) -> Result<()> {
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED)?;
    }

    // TODO: remove
    println!(
        "Using index \"{}\" and path \"{}\".",
        display_index, output_path
    );

    // Get the display handle using the provided index
    let display_handle = get_display_handle_from_index(display_index)
        .expect("The provided display index was out of bounds!");
    let item = create_capture_item_for_monitor(display_handle)?;

    // TODO: Make these encoding settings configurable
    let encoder_devices = VideoEncoderDevice::enumerate()?;
    println!("Encoders ({}):", encoder_devices.len());
    for encoder_device in &encoder_devices {
        println!("  {}", encoder_device.display_name());
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
            Arg::with_name("OUTPUT FILE")
                .help("The output file that will contain the recording.")
                .default_value("recording.mp4")
                .required(false),
        )
        .get_matches();

    let monitor_index: usize = matches.value_of("display").unwrap().parse().unwrap();
    let output_path = matches.value_of("OUTPUT FILE").unwrap();

    let result = run(monitor_index, output_path);

    // We do this for nicer HRESULT printing when errors occur.
    if let Err(error) = result {
        error.code().unwrap();
    }
}
