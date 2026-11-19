//! A picture of the interface saved as a PNG without opening a window: the same
//! drawing on a bitmap, encoded by WIC. It is how the design is checked (and
//! compared with the reference) from a script.

use std::path::Path;
use std::time::Duration;

use purifex_domain::ui_event::UiHandler;
use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::GENERIC_WRITE;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppPBGRA, IWICBitmap,
    IWICBitmapSource, IWICImagingFactory, WICBitmapCacheOnDemand, WICBitmapEncoderNoCache,
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
    let size = (
        (width * scale).ceil() as u32,
        (height * scale).ceil() as u32,
    );
    // SAFETY: COM is started for this thread; every object below is released when dropped.
    let wic: IWICImagingFactory = unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("no se pudo iniciar WIC: {e}"))?
    };
    let bitmap = draw_on_bitmap(&wic, size, (width, height), scale, handler)?;
    write_png(&wic, &bitmap, size, path)
}

/// Draws the picture of `handler` on a new bitmap of `size` pixels.
fn draw_on_bitmap(
    wic: &IWICImagingFactory,
    size: (u32, u32),
    units: (f32, f32),
    scale: f32,
    handler: &mut dyn UiHandler,
) -> Result<IWICBitmap, String> {
    // SAFETY: the bitmap is created with a valid size and format.
    let bitmap = unsafe {
        wic.CreateBitmap(
            size.0,
            size.1,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapCacheOnDemand,
        )
    }
    .map_err(|e| e.to_string())?;
    let mut renderer = Renderer::for_bitmap(&bitmap, 96.0 * scale)?;
    // Two pictures: the first starts the animations, the second is long after they
    // ended, so a screenshot shows the page as it settles.
    handler.frame(units.0, units.1, renderer.text(), Duration::ZERO);
    let list = handler.frame(units.0, units.1, renderer.text(), Duration::from_secs(60));
    if renderer.draw(&list)? == Drawn::Recreate {
        return Err("se perdió el dispositivo gráfico".into());
    }
    Ok(bitmap)
}

/// Encodes the bitmap as a PNG file at `path`.
fn write_png(
    wic: &IWICImagingFactory,
    bitmap: &IWICBitmap,
    size: (u32, u32),
    path: &Path,
) -> Result<(), String> {
    let wide: Vec<u16> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `wide` is NUL-terminated and outlives the calls; the objects are WIC's, live
    // until they are dropped; the frame is committed before the encoder.
    unsafe {
        let encoder = wic
            .CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())
            .map_err(|e| e.to_string())?;
        let stream = wic.CreateStream().map_err(|e| e.to_string())?;
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
        frame.SetSize(size.0, size.1).map_err(|e| e.to_string())?;
        let mut format = GUID_WICPixelFormat32bppPBGRA;
        frame
            .SetPixelFormat(&mut format)
            .map_err(|e| e.to_string())?;
        let source = bitmap
            .cast::<IWICBitmapSource>()
            .map_err(|e| e.to_string())?;
        frame
            .WriteSource(&source, std::ptr::null())
            .map_err(|e| e.to_string())?;
        frame.Commit().map_err(|e| e.to_string())?;
        encoder.Commit().map_err(|e| e.to_string())?;
    }
    Ok(())
}
