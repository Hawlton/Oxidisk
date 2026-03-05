use windows::{
    core::*,
    Win32::System::Com::*,
    Win32::Storage::Imapi::*,
};

// Note: IStream import removed as it's unused

pub struct DriveInfo {
    pub id: String,
    pub media_label: String,
    pub media_type: Option<String>,
    pub capacity: Option<u128>,
    pub capacity_readble: Option<String> 
}

impl DriveInfo{
    fn new(device_id: &String, label: &String) -> DriveInfo{
        DriveInfo{
            id: device_id.to_string(),
            media_label: label.to_string(),
            media_type: None,
            capacity: None,
            capacity_readble: None

        }
    }

    pub fn get_media_type(media_num: IMAPI_MEDIA_PHYSICAL_TYPE) -> String {
        match media_num {
            IMAPI_MEDIA_TYPE_UNKNOWN => "Unknown".to_string(),
            IMAPI_MEDIA_TYPE_CDROM => "CDROM".to_string(),
            IMAPI_MEDIA_TYPE_CDR => "CDR".to_string(),
            IMAPI_MEDIA_TYPE_CDRW => "CDRW".to_string(),
            IMAPI_MEDIA_TYPE_DVDROM => "DVDROM".to_string(),
            IMAPI_MEDIA_TYPE_DVDRAM => "DVDRAM".to_string(),
            IMAPI_MEDIA_TYPE_DVDPLUSR => "DVD+R".to_string(),
            IMAPI_MEDIA_TYPE_DVDPLUSRW => "DVD+RW".to_string(),
            IMAPI_MEDIA_TYPE_DVDPLUSRW_DUALLAYER => "DVD+RW_DL".to_string(),
            IMAPI_MEDIA_TYPE_DVDPLUSR_DUALLAYER => "DVD+R_DL".to_string(),
            IMAPI_MEDIA_TYPE_DISK => "Disk".to_string(),
            IMAPI_MEDIA_TYPE_BDR => "BD-R".to_string(),
            IMAPI_MEDIA_TYPE_BDRE => "BD-RE".to_string(),
            IMAPI_MEDIA_TYPE_BDROM => "BD-ROM".to_string(),
            _ => "Unknown".to_string(),
        }
    }

    fn human_readable(bytes: u128 ) -> String{
        const SUFFIXES: &[&str] = &["B", "KB", "MB", "GB"];
        let mut i = 0;
        //buffer overflow waiting to happen...TOO BAD
        let mut double_bytes: f64 = bytes as f64;
        while double_bytes > 1024 as f64 && i != SUFFIXES.len() {
            double_bytes /= 1024.0;
            i += 1;
        }
        double_bytes.to_string();
        return format!("{:.2} {}", double_bytes, &SUFFIXES[i])
    }
}

pub fn list_drives() -> Result<Vec<DriveInfo>> {
    let mut drive_list = Vec::new();

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        
        // Scope to ensure all COM objects are dropped before CoUninitialize
        {
            let disc_master: IDiscMaster2 = CoCreateInstance(&MsftDiscMaster2, None, CLSCTX_ALL)?;
            for i in 0..disc_master.Count()? {
                let unique_id = disc_master.get_Item(i)?;
                let recorder: IDiscRecorder2 = CoCreateInstance(&MsftDiscRecorder2, None, CLSCTX_ALL )?;
                let format: IDiscFormat2Data = CoCreateInstance(&MsftDiscFormat2Data, None, CLSCTX_ALL )?;
                recorder.InitializeDiscRecorder(&unique_id)?;
                if !bool::from(format.IsRecorderSupported(&recorder).expect("plz work")) { continue; }
                if !bool::from(format.IsCurrentMediaSupported(&recorder).expect("y u no work")) { continue; }
                format.SetRecorder(&recorder)?;
                
                let label = format!("{}",  recorder.ProductId()?).trim().to_string();
                let mut new_drive = DriveInfo::new(&unique_id.to_string(), &label);

                //this check might be redundant, but who knows
                if let Ok(media_index) = format.CurrentPhysicalMediaType() {
                    new_drive.media_type = Some(DriveInfo::get_media_type(media_index));
                } else {
                    new_drive.media_type = Some("No Disk".to_string());
                    new_drive.capacity = Some(0);
                    new_drive.capacity_readble = Some("N/A".to_string());
                    drive_list.push(new_drive);
                    continue;
                }

                let drive_cap: u128 = format.FreeSectorsOnMedia().expect("getting capacity should work") as u128 * 2048;
                new_drive.capacity = Some(drive_cap);
                new_drive.capacity_readble = Some(DriveInfo::human_readable(drive_cap));
                drive_list.push(new_drive);
            }
        } 
        CoUninitialize();
    }
    Ok(drive_list)
}

    