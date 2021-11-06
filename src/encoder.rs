use windows::{
    runtime::{Interface, Result, GUID},
    Win32::{
        Foundation::PWSTR,
        Media::MediaFoundation::{
            IMFActivate, IMFAttributes, MFMediaType_Video, MFTEnumEx, MFT_FRIENDLY_NAME_Attribute,
            MFVideoFormat_H264, MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_HARDWARE,
            MFT_ENUM_FLAG_SORTANDFILTER, MFT_ENUM_FLAG_TRANSCODE_ONLY, MFT_REGISTER_TYPE_INFO,
            MF_E_ATTRIBUTENOTFOUND,
        },
        System::Com::CoTaskMemFree,
    },
};

pub struct VideoEncoderDevice {
    source: IMFActivate,
    display_name: String,
}

impl VideoEncoderDevice {
    pub fn enumerate() -> Result<Vec<VideoEncoderDevice>> {
        let output_info = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Video,
            guidSubtype: MFVideoFormat_H264,
        };
        let encoders = enumerate_mfts(
            &MFT_CATEGORY_VIDEO_ENCODER,
            (MFT_ENUM_FLAG_HARDWARE.0
                | MFT_ENUM_FLAG_TRANSCODE_ONLY.0
                | MFT_ENUM_FLAG_SORTANDFILTER.0) as u32,
            None,
            Some(&output_info),
        )?;
        let mut encoder_devices = Vec::new();
        for encoder in encoders {
            let display_name = if let Some(display_name) =
                get_string_attribute(&encoder.cast()?, &MFT_FRIENDLY_NAME_Attribute)?
            {
                display_name
            } else {
                "Unknown".to_owned()
            };
            let encoder_device = VideoEncoderDevice {
                source: encoder,
                display_name,
            };
            encoder_devices.push(encoder_device);
        }
        Ok(encoder_devices)
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

fn type_info_to_ptr(type_info: Option<&MFT_REGISTER_TYPE_INFO>) -> *const MFT_REGISTER_TYPE_INFO {
    if let Some(type_info) = type_info {
        type_info as *const _
    } else {
        std::ptr::null()
    }
}

fn enumerate_mfts(
    category: &GUID,
    flags: u32,
    input_type: Option<&MFT_REGISTER_TYPE_INFO>,
    output_type: Option<&MFT_REGISTER_TYPE_INFO>,
) -> Result<Vec<IMFActivate>> {
    let mut transform_sources = Vec::new();
    let mut mfactivate_list = std::ptr::null_mut();
    let mut num_mfactivate = 0;
    unsafe {
        MFTEnumEx(
            category,
            flags,
            type_info_to_ptr(input_type),
            type_info_to_ptr(output_type),
            &mut mfactivate_list,
            &mut num_mfactivate,
        )?;
    }
    if num_mfactivate > 0 {
        unsafe {
            // If we have more than one IMFActivate in the list,
            // we can transmute it out of the Option<_>
            let mfactivate_list: *mut IMFActivate = std::mem::transmute(mfactivate_list);
            let mfactivate_slice =
                std::slice::from_raw_parts(mfactivate_list, num_mfactivate as usize);
            for mfactivate in mfactivate_slice {
                let transform_source = mfactivate.clone();
                transform_sources.push(transform_source);
                // We need to release each item
                std::mem::drop(mfactivate)
            }
            // Free the memory that was allocated for the list
            CoTaskMemFree(mfactivate_list as *const _);
        }
    }
    Ok(transform_sources)
}

fn get_string_attribute(
    attributes: &IMFAttributes,
    attribute_guid: &GUID,
) -> Result<Option<String>> {
    unsafe {
        match attributes.GetStringLength(attribute_guid) {
            Ok(mut length) => {
                let mut result = vec![0u16; (length + 1) as usize];
                attributes.GetString(
                    attribute_guid,
                    PWSTR(result.as_ptr() as *mut _),
                    result.len() as u32,
                    &mut length,
                )?;
                result.resize(length as usize, 0);
                Ok(Some(String::from_utf16(&result).unwrap()))
            }
            Err(error) => {
                if error.code() == MF_E_ATTRIBUTENOTFOUND {
                    Ok(None)
                } else {
                    Err(error)
                }
            }
        }
    }
}
