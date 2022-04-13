use windows::{
    core::{Result, GUID},
    Win32::{
        Media::MediaFoundation::{
            IMFActivate, IMFAttributes, MFTEnumEx, MFT_REGISTER_TYPE_INFO, MF_E_ATTRIBUTENOTFOUND,
        },
        System::Com::CoTaskMemFree,
    },
};

fn type_info_to_ptr(type_info: Option<&MFT_REGISTER_TYPE_INFO>) -> *const MFT_REGISTER_TYPE_INFO {
    if let Some(type_info) = type_info {
        type_info as *const _
    } else {
        std::ptr::null()
    }
}

pub fn enumerate_mfts(
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
                // This is a dirty trick we play so that we can
                // release the underlying IMFActivate despite having
                // a shared reference.
                let temp: windows::core::IUnknown = std::mem::transmute_copy(&transform_source);
                transform_sources.push(transform_source);
                // We need to release each item
                std::mem::drop(temp)
            }
            // Free the memory that was allocated for the list
            CoTaskMemFree(mfactivate_list as *const _);
        }
    }
    Ok(transform_sources)
}

pub fn get_string_attribute(
    attributes: &IMFAttributes,
    attribute_guid: &GUID,
) -> Result<Option<String>> {
    unsafe {
        match attributes.GetStringLength(attribute_guid) {
            Ok(mut length) => {
                let mut result = vec![0u16; (length + 1) as usize];
                attributes.GetString(attribute_guid, &mut result, &mut length)?;
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

// These inlined helpers aren't represented in the metadata

// This is the value for Win7+
pub const MF_VERSION: u32 = 131184;

fn pack_2_u32_as_u64(high: u32, low: u32) -> u64 {
    ((high as u64) << 32) | low as u64
}

#[allow(non_snake_case)]
unsafe fn MFSetAttribute2UINT32asUINT64(
    attributes: &IMFAttributes,
    key: &GUID,
    high: u32,
    low: u32,
) -> Result<()> {
    attributes.SetUINT64(key, pack_2_u32_as_u64(high, low))
}

#[allow(non_snake_case)]
pub unsafe fn MFSetAttributeSize(
    attributes: &IMFAttributes,
    key: &GUID,
    width: u32,
    height: u32,
) -> Result<()> {
    MFSetAttribute2UINT32asUINT64(attributes, key, width, height)
}

#[allow(non_snake_case)]
pub unsafe fn MFSetAttributeRatio(
    attributes: &IMFAttributes,
    key: &GUID,
    numerator: u32,
    denominator: u32,
) -> Result<()> {
    MFSetAttribute2UINT32asUINT64(attributes, key, numerator, denominator)
}
