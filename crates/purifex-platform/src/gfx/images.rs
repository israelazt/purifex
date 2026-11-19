//! The pictures of the program (the logo): PNG files compiled into the program,
//! decoded by WIC the first time they are drawn and kept as Direct2D bitmaps.

use windows::Win32::Graphics::Direct2D::{ID2D1Bitmap, ID2D1RenderTarget};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};

/// The bytes of a picture by name (the files of `assets/`).
pub fn bytes(name: &str) -> Option<&'static [u8]> {
    match name {
        "logo-dark-mode" => Some(include_bytes!("../../../../assets/logo-dark-mode.png")),
        "logo-light-mode" => Some(include_bytes!("../../../../assets/logo-light-mode.png")),
        _ => None,
    }
}

/// Decodes a PNG into a bitmap of `target`.
pub fn decode(target: &ID2D1RenderTarget, png: &[u8]) -> Result<ID2D1Bitmap, String> {
    let fail = |what: &'static str| move |e: windows::core::Error| format!("{what}: {e}");
    // SAFETY: plain WIC calls on objects created here; `png` outlives the stream
    // because the bitmap is created before this function returns.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
                .map_err(fail("no se pudo iniciar WIC"))?;
        let stream = wic.CreateStream().map_err(fail("WIC: flujo"))?;
        stream
            .InitializeFromMemory(png)
            .map_err(fail("WIC: memoria"))?;
        let decoder = wic
            .CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)
            .map_err(fail("WIC: decodificador"))?;
        let frame = decoder.GetFrame(0).map_err(fail("WIC: cuadro"))?;
        let converter = wic
            .CreateFormatConverter()
            .map_err(fail("WIC: conversor"))?;
        converter
            .Initialize(
                &frame,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeMedianCut,
            )
            .map_err(fail("WIC: formato"))?;
        target
            .CreateBitmapFromWicBitmap(&converter, None)
            .map_err(fail("Direct2D: imagen"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

    #[test]
    fn both_logos_are_compiled_in_as_pngs_and_an_unknown_name_is_nothing() {
        for name in ["logo-dark-mode", "logo-light-mode"] {
            let data = bytes(name).unwrap_or_else(|| panic!("{name} missing"));
            assert_eq!(data[..8], PNG_SIGNATURE, "{name} is not a PNG");
        }
        assert!(bytes("logo-neon").is_none());
    }

    #[test]
    fn the_two_logos_are_different_pictures() {
        assert_ne!(bytes("logo-dark-mode"), bytes("logo-light-mode"));
    }
}
