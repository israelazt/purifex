//! A picture of the interface saved as a PNG without opening a window: the same
//! drawing on a bitmap, encoded by WIC. It is how the design is checked (and
//! compared with the reference) from a script.

use std::path::Path;

use purifex_domain::ui_event::UiHandler;
use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::GENERIC_WRITE;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppPBGRA,
    IWICImagingFactory, WICBitmapCacheOnDemand, WICBitmapEncoderNoCache,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};

use super::{Drawn, Renderer};

/// Draws what `handler` shows at `width` x `height` units, at `scale` (1.0 = 96
/// dpi), and writes it to `path` as a PNG.
pub fn render_png(
    path: &Path,
    width: f32,
    height: f32,
    scale: f32,
    handler: &mut dyn UiHandler,
) -> Result<(), String> {
    let (w, h) = (
        (width * scale).ceil() as u32,
        (height * scale).ceil() as u32,
    );
    // SAFETY: COM is started for this thread; every object below is released when dropped.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| format!("no se pudo iniciar WIC: {e}"))?;
        let bitmap = wic
            .CreateBitmap(w, h, &GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnDemand)
            .map_err(|e| e.to_string())?;
        let mut renderer = Renderer::for_bitmap(&bitmap, 96.0 * scale)?;
        let list = handler.frame(width, height, renderer.text());
        if renderer.draw(&list)? == Drawn::Recreate {
            return Err("se perdió el dispositivo gráfico".into());
        }

        let encoder = wic
            .CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())
            .map_err(|e| e.to_string())?;
        let stream = wic.CreateStream().map_err(|e| e.to_string())?;
        let wide: Vec<u16> = path
            .as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        stream
            .InitializeFromFilename(PCWSTR(wide.as_ptr()), GENERIC_WRITE.0)
            .map_err(|e| format!("no se pudo crear {}: {e}", path.display()))?;
        encoder
            .Initialize(&stream, WICBitmapEncoderNoCache)
            .map_err(|e| e.to_string())?;
        let (mut frame, mut options) = (None, None);
        encoder
            .CreateNewFrame(&mut frame, &mut options)
            .map_err(|e| e.to_string())?;
        let frame = frame.ok_or("WIC no dio un cuadro")?;
        frame
            .Initialize(options.as_ref())
            .map_err(|e| e.to_string())?;
        frame.SetSize(w, h).map_err(|e| e.to_string())?;
        let mut format = GUID_WICPixelFormat32bppPBGRA;
        frame
            .SetPixelFormat(&mut format)
            .map_err(|e| e.to_string())?;
        frame
            .WriteSource(
                &bitmap
                    .cast::<windows::Win32::Graphics::Imaging::IWICBitmapSource>()
                    .map_err(|e| e.to_string())?,
                std::ptr::null(),
            )
            .map_err(|e| e.to_string())?;
        frame.Commit().map_err(|e| e.to_string())?;
        encoder.Commit().map_err(|e| e.to_string())?;
    }
    Ok(())
}
