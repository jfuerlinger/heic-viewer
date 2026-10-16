//! Usage:
//!   thumbnail-probe [--shell] <file.heic> [size] [--bmp out.bmp]
//!
//! Default mode: CoCreateInstance on the registered CLSID (checks the installer's COM
//! registration and that heif.dll/libde265.dll load from the install directory).
//! --shell: asks the Windows shell for the thumbnail (IShellItemImageFactory), i.e. the same
//! association lookup and out-of-process host that Explorer uses.
//! Exit code 0 = thumbnail produced, 1 = failure.

#[cfg(not(windows))]
fn main() {
    eprintln!("thumbnail-probe only works on Windows");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    match probe::run() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(windows)]
mod probe {
    use std::ffi::c_void;

    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{DeleteObject, GetObjectW, BITMAP, HBITMAP, HGDIOBJ};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ,
        STGM_SHARE_DENY_WRITE,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::IInitializeWithStream;
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, IThumbnailProvider, SHCreateItemFromParsingName, SHCreateStreamOnFileEx,
        SIIGBF_THUMBNAILONLY, WTSAT_UNKNOWN,
    };
    use windows_core::{IUnknown, Interface, GUID, HSTRING};

    const CLSID: GUID = GUID::from_u128(0x218c7ae3_fdc4_4381_abcd_d7385afb7a4a);

    pub fn run() -> Result<(), String> {
        let mut args: Vec<String> = std::env::args().skip(1).collect();
        let shell = take_flag(&mut args, "--shell");
        let bmp_out = take_value(&mut args, "--bmp");
        let file = args.first().ok_or("usage: thumbnail-probe [--shell] <file> [size] [--bmp out.bmp]")?.clone();
        let size: u32 = args.get(1).map(|s| s.parse().map_err(|_| "size must be a number")).transpose()?.unwrap_or(256);
        let path = std::fs::canonicalize(&file).map_err(|e| format!("{file}: {e}"))?;
        let path = path.to_string_lossy().trim_start_matches(r"\\?\").to_string();

        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let (bitmap, alpha) = if shell {
                let item: IShellItemImageFactory =
                    SHCreateItemFromParsingName(&HSTRING::from(&path), None).map_err(|e| format!("shell item: {e}"))?;
                let size = SIZE { cx: size as i32, cy: size as i32 };
                (item.GetImage(size, SIIGBF_THUMBNAILONLY).map_err(|e| format!("shell GetImage: {e}"))?, None)
            } else {
                let provider: IThumbnailProvider = CoCreateInstance(&CLSID, None::<&IUnknown>, CLSCTX_INPROC_SERVER)
                    .map_err(|e| format!("CoCreateInstance (is the handler installed?): {e}"))?;
                let stream = SHCreateStreamOnFileEx(
                    &HSTRING::from(&path),
                    (STGM_READ | STGM_SHARE_DENY_WRITE).0,
                    0,
                    false,
                    None,
                )
                .map_err(|e| format!("open: {e}"))?;
                provider
                    .cast::<IInitializeWithStream>()
                    .and_then(|i| i.Initialize(&stream, STGM_READ.0))
                    .map_err(|e| format!("Initialize: {e}"))?;
                let mut bitmap = HBITMAP::default();
                let mut alpha = WTSAT_UNKNOWN;
                provider.GetThumbnail(size, &mut bitmap, &mut alpha).map_err(|e| format!("GetThumbnail: {e}"))?;
                (bitmap, Some(alpha.0))
            };
            let result = describe(bitmap, alpha, bmp_out.as_deref());
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            result
        }
    }

    unsafe fn describe(bitmap: HBITMAP, alpha: Option<i32>, bmp_out: Option<&str>) -> Result<(), String> {
        let mut bm = BITMAP::default();
        if unsafe {
            GetObjectW(HGDIOBJ(bitmap.0), std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as *mut c_void))
        } == 0
        {
            return Err("GetObject failed".into());
        }
        print!("{}x{} {}bpp", bm.bmWidth, bm.bmHeight.abs(), bm.bmBitsPixel);
        if let Some(a) = alpha {
            print!(" alpha={}", ["unknown", "rgb", "argb"].get(a as usize).unwrap_or(&"?"));
        }
        if bm.bmBits.is_null() || bm.bmBitsPixel != 32 {
            println!();
            return Ok(());
        }
        let len = (bm.bmWidthBytes * bm.bmHeight.abs()) as usize;
        let bits = unsafe { std::slice::from_raw_parts(bm.bmBits as *const u8, len) };
        let center = ((bm.bmHeight.abs() / 2) * bm.bmWidthBytes + (bm.bmWidth / 2) * 4) as usize;
        println!(" center=#{:02X}{:02X}{:02X}", bits[center + 2], bits[center + 1], bits[center]);
        if let Some(out) = bmp_out {
            write_bmp(out, bm.bmWidth, bm.bmHeight, bits).map_err(|e| format!("{out}: {e}"))?;
            println!("written {out}");
        }
        Ok(())
    }

    /// 32-bit BMP; a negative height keeps the DIB's top-down row order.
    fn write_bmp(path: &str, width: i32, height: i32, bits: &[u8]) -> std::io::Result<()> {
        let mut f = Vec::with_capacity(54 + bits.len());
        f.extend_from_slice(b"BM");
        f.extend_from_slice(&((54 + bits.len()) as u32).to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&54u32.to_le_bytes());
        f.extend_from_slice(&40u32.to_le_bytes());
        f.extend_from_slice(&width.to_le_bytes());
        f.extend_from_slice(&height.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&32u16.to_le_bytes());
        f.extend_from_slice(&[0u8; 24]);
        f.extend_from_slice(bits);
        std::fs::write(path, f)
    }

    fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
        let before = args.len();
        args.retain(|a| a != flag);
        args.len() != before
    }

    fn take_value(args: &mut Vec<String>, flag: &str) -> Option<String> {
        let i = args.iter().position(|a| a == flag)?;
        args.remove(i);
        (i < args.len()).then(|| args.remove(i))
    }
}
