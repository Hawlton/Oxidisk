use std::path::Path;
use std::thread;
use std::sync::mpsc::Sender;
use crate::hw_enum;

use windows::Win32::Foundation::{FVE_E_INVALID_NBP_CERT, VARIANT_FALSE};
use windows::Win32::UI::Shell::SHCreateStreamOnFileEx;
use windows::Win32::Storage::Imapi::*;
use windows::core::{BSTR, ComInterface, HRESULT, HSTRING, Error, Result, implement};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    IConnectionPointContainer, IDispatch, IDispatch_Impl, STGM_READ, STGM_SHARE_DENY_WRITE
};

pub enum BurnEvent{
    Error(String),
    Log(String),
    Progress(f32),
    Finished,
}


//this makes me want to suck-start a shotgun.
#[implement(IDispatch, DDiscFormat2DataEvents)]
struct ComSink{
    tx: Sender<BurnEvent>,
}

impl IDispatch_Impl for ComSink {
    fn GetTypeInfoCount(&self) -> Result<u32> { Ok(0) }
    fn GetTypeInfo(&self, _: u32, _: u32) -> Result<windows::Win32::System::Com::ITypeInfo> {
        Err(Error::from(HRESULT(0x80004001u32 as i32)))
    }
    fn GetIDsOfNames(&self, _: *const windows::core::GUID, _: *const windows::core::PCWSTR, _: u32, _: u32, _: *mut i32) -> Result<()> {
        Err(Error::from(HRESULT(0x80004001u32 as i32)))
    }
    fn Invoke(&self, _: i32, _: *const windows::core::GUID, _: u32, _: windows::Win32::System::Com::DISPATCH_FLAGS, _: *const windows::Win32::System::Com::DISPPARAMS, _: *mut windows::Win32::System::Variant::VARIANT, _: *mut windows::Win32::System::Com::EXCEPINFO, _: *mut u32) -> Result<()> {
        Ok(())
    }
}

impl DDiscFormat2DataEvents_Impl for ComSink {
    fn Update(&self, _object: Option<&IDispatch>, progress: Option<&IDispatch>) -> Result<()> {
        if let Some(disp) = progress {
            if let Ok(event_args) = disp.cast::<IDiscFormat2DataEventArgs>() {
                unsafe {
                    let lba = event_args.LastWrittenLba().unwrap_or(0);
                    let total = event_args.SectorCount().unwrap_or(1);
                    if total > 0 {
                        let pct = (lba as f32) / (total as f32);
                        let _ = self.tx.send(BurnEvent::Progress(pct));
                        let _ = self.tx.send(BurnEvent::Log(format!("percent: {}", pct.to_string())));
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn spawn_burn_thread(file_list: &[String], vl: &str, drive: &str, tx: Sender<BurnEvent>, finalize: bool) {
    let owned_files = file_list.to_vec();
    let owned_label = vl.to_string();
    let owned_drive = drive.to_string();
    thread::spawn(move || {
        unsafe {
            if let Err(e) = CoInitializeEx(None, COINIT_APARTMENTTHREADED) {
                let _ = tx.send(BurnEvent::Error(format!("Could not spawn apartment thread: {e:?}")));
                return;
            }
        }
        match burn_logic(owned_files, owned_label, owned_drive, &tx, finalize) {
            Ok(_) => {let _ = tx.send(BurnEvent::Finished);}
            Err(e) => {
                let _ = tx.send(BurnEvent::Error(format!("Burn Failed: 0x{:08X}", e.code().0)));
                let _ = tx.send(BurnEvent::Error(format!("Press 'esc' to return to Main Menu")));
            }
        }
    });
}

fn burn_logic(file_list: Vec<String>, vl: String, drive_id: String, tx: &Sender<BurnEvent>, finalize: bool) -> Result<()> {
    unsafe {
        //let disc_master: IDiscMaster2 = CoCreateInstance(&MsftDiscMaster2, None, CLSCTX_ALL)?;
        let unique_id = &BSTR::from(drive_id);
        let disc_recorder: IDiscRecorder2 = CoCreateInstance(&MsftDiscRecorder2, None, CLSCTX_ALL)?;
        let disc_format: IDiscFormat2Data = CoCreateInstance(&MsftDiscFormat2Data, None, CLSCTX_ALL)?;
        disc_recorder.InitializeDiscRecorder(unique_id)?;
        disc_format.SetClientName(&BSTR::from("Oxidisk"))?;
        let _ = tx.send(BurnEvent::Log(format!("COM Objects initialized")));
        
        if file_list.is_empty() {
            let _ = tx.send(BurnEvent::Error(format!("File list is empty")));
            return Err(Error::from(HRESULT(0x80070057u32 as i32))); 
        }
        if disc_format.IsRecorderSupported(&disc_recorder)? == VARIANT_FALSE {
            let _ = tx.send(BurnEvent::Error(format!("Recorder is not supported. Make sure you have the correct drive selected")));
            let product_id = disc_recorder.ProductId()?;
            let _ = tx.send(BurnEvent::Error(format!("Product ID: {}", product_id)));
            return Err(Error::from(HRESULT(0xC0AA0407u32 as i32)));
        }
        if disc_format.IsCurrentMediaSupported(&disc_recorder)? == VARIANT_FALSE {
            let _ = tx.send(BurnEvent::Error(format!("Media is not supported. Make sure media is writable and blank")));
            return Err(Error::from(HRESULT(0xC0AA0406u32 as i32)));
        }

        
        let _ = tx.send(BurnEvent::Log(format!("Setting recorder")));
        disc_format.SetRecorder(&disc_recorder)?;      

        let _ = tx.send(BurnEvent::Log(format!("Setting up progress events")));
        let container: IConnectionPointContainer = disc_format.cast()?;
        let events_guid = DDiscFormat2DataEvents::IID;
        let connection_point = container.FindConnectionPoint(&events_guid)?;
        let sink = ComSink { tx: tx.clone() };
        let sink_interface: DDiscFormat2DataEvents = sink.into();
        let cookie = connection_point.Advise(&sink_interface)?;

        let _ = tx.send(BurnEvent::Log(format!("Creating File System")));
        let media_type = disc_format.CurrentPhysicalMediaType()?;
        let _ = tx.send(BurnEvent::Log(format!("Current Media type: {:?}", hw_enum::DriveInfo::get_media_type(media_type))));
        let fs_image: IFileSystemImage = CoCreateInstance(&MsftFileSystemImage, None, CLSCTX_ALL)?;
        let _ = tx.send(BurnEvent::Log(format!("Created FS Image")));
        let file_sytems = match media_type {
            IMAPI_MEDIA_TYPE_CDR | IMAPI_MEDIA_TYPE_CDRW => {FsiFileSystems(FsiFileSystemISO9660.0 | FsiFileSystemJoliet.0)}

            IMAPI_MEDIA_TYPE_DVDRAM | IMAPI_MEDIA_TYPE_DVDPLUSR | IMAPI_MEDIA_TYPE_DVDPLUSRW |
            IMAPI_MEDIA_TYPE_DVDPLUSR_DUALLAYER | IMAPI_MEDIA_TYPE_DVDDASHR | IMAPI_MEDIA_TYPE_DVDDASHRW |
            IMAPI_MEDIA_TYPE_DVDDASHR_DUALLAYER => {FsiFileSystemUDF}

            IMAPI_MEDIA_TYPE_BDR | IMAPI_MEDIA_TYPE_BDRE => {FsiFileSystemUDF}
            _ => FsiFileSystemUDF
        };

        fs_image.SetVolumeName(&BSTR::from(vl))?;
        fs_image.SetFileSystemsToCreate(file_sytems)?;
        let root: IFsiDirectoryItem = fs_image.Root()?;

        let _ = tx.send(BurnEvent::Log(format!("Adding files to image")));
        for file in &file_list {
            let path = Path::new(file);
            if !Path::exists(path) {
                let _ = tx.send(BurnEvent::Log(format!("Path: {} does not exist. Skipping", file)));
                continue;
            }
            if path.is_dir() {
                let dir_path = BSTR::from(path.to_string_lossy().to_string());
                root.AddDirectory(&dir_path)?;
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let stream = SHCreateStreamOnFileEx(&HSTRING::from(file), STGM_READ.0 | STGM_SHARE_DENY_WRITE.0, 0, false, None)?;
            root.AddFile(&BSTR::from(format!("\\{}", name)), &stream)?;
        }

        let _ = tx.send(BurnEvent::Log(format!("Creating image stream")));
        let result_image = fs_image.CreateResultImage()?;
        let image_stream = result_image.ImageStream()?;
        let disc_capacity = disc_format.FreeSectorsOnMedia()?;
        if disc_capacity < result_image.BlockSize()?{
            let _= tx.send(BurnEvent::Error(format!("Burn job size exceeds disc capacity")));
            return Err(Error::from(HRESULT(0xC0AA0404u32 as i32)));
        }
        if finalize {disc_format.ForceMediaToBeClosed();}
        let _ = tx.send(BurnEvent::Log(format!("Beginning Write")));
        let write_result = disc_format.Write(&image_stream);
        let _ = connection_point.Unadvise(cookie);
        write_result?;
    }
    Ok(())
}