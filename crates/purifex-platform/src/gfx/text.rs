//! Text with DirectWrite: the fonts of the design (a monospace one for data, a
//! sans one for headings, each with the best the system has), measuring for the
//! layout, and drawing with the same numbers so what is measured is what is seen.

use std::cell::RefCell;
use std::collections::HashMap;

use purifex_domain::draw::{Font, Size, TextStyle};
use purifex_domain::ports::TextMetrics;
use windows::core::{Interface, PCWSTR};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout, IDWriteTextLayout1,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_METRICS, DWRITE_TEXT_RANGE,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
};

/// The families tried in order for each typeface.
const MONO: [&str; 3] = ["Cascadia Mono", "Consolas", "Courier New"];
const SANS: [&str; 3] = ["Segoe UI Variable Display", "Segoe UI", "Arial"];
/// The most measures kept; the cache starts over past it.
const MEASURES_KEPT: usize = 4096;

type FormatKey = (bool, u32, bool);
type MeasureKey = (String, FormatKey, u32);

pub struct TextEngine {
    factory: IDWriteFactory,
    mono: Vec<u16>,
    sans: Vec<u16>,
    formats: RefCell<HashMap<FormatKey, IDWriteTextFormat>>,
    measured: RefCell<HashMap<MeasureKey, Size>>,
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The first family of `candidates` that the system has (the last one if none).
fn installed(factory: &IDWriteFactory, candidates: &[&str]) -> Vec<u16> {
    let mut collection = None;
    // SAFETY: a plain query; the collection is an output.
    let _ = unsafe { factory.GetSystemFontCollection(&mut collection, false) };
    if let Some(collection) = collection {
        for name in candidates {
            let (mut index, mut exists) = (0u32, false.into());
            let family = wide(name);
            // SAFETY: the name is NUL-ended and the outputs are live locals.
            let found = unsafe {
                collection.FindFamilyName(PCWSTR(family.as_ptr()), &mut index, &mut exists)
            };
            if found.is_ok() && exists.as_bool() {
                return family;
            }
        }
    }
    wide(candidates[candidates.len() - 1])
}

impl TextEngine {
    pub fn new() -> Result<Self, String> {
        // SAFETY: creates the shared factory of the system.
        let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }
            .map_err(|e| format!("no se pudo iniciar DirectWrite: {e}"))?;
        Ok(Self {
            mono: installed(&factory, &MONO),
            sans: installed(&factory, &SANS),
            factory,
            formats: RefCell::new(HashMap::new()),
            measured: RefCell::new(HashMap::new()),
        })
    }

    fn format(&self, style: &TextStyle) -> Result<IDWriteTextFormat, String> {
        let key = (style.font == Font::Mono, style.size.to_bits(), style.bold);
        if let Some(format) = self.formats.borrow().get(&key) {
            return Ok(format.clone());
        }
        let family = if style.font == Font::Mono {
            &self.mono
        } else {
            &self.sans
        };
        let weight = if style.bold {
            DWRITE_FONT_WEIGHT_BOLD
        } else {
            DWRITE_FONT_WEIGHT_NORMAL
        };
        // SAFETY: the family and the locale are NUL-ended and outlive the call.
        let format = unsafe {
            self.factory.CreateTextFormat(
                PCWSTR(family.as_ptr()),
                None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                style.size,
                windows::core::w!("es-ES"),
            )
        }
        .map_err(|e| format!("no se pudo crear la fuente: {e}"))?;
        // SAFETY: plain setters on the format just created.
        unsafe {
            let _ = format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
        }
        self.formats.borrow_mut().insert(key, format.clone());
        Ok(format)
    }

    /// One line of text ready to draw, cut with `…` at `max_width` if given.
    pub fn layout(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
    ) -> Result<IDWriteTextLayout, String> {
        let format = self.format(style)?;
        let units: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: the text is a live slice; the format was made by this factory.
        let layout = unsafe {
            self.factory
                .CreateTextLayout(&units, &format, max_width.unwrap_or(100_000.0), 1000.0)
        }
        .map_err(|e| format!("no se pudo componer el texto: {e}"))?;
        // SAFETY: setters on the layout just created, with live arguments.
        unsafe {
            if style.tracking != 0.0 {
                if let Ok(spaced) = layout.cast::<IDWriteTextLayout1>() {
                    let _ = spaced.SetCharacterSpacing(
                        0.0,
                        style.tracking,
                        0.0,
                        DWRITE_TEXT_RANGE {
                            startPosition: 0,
                            length: units.len() as u32,
                        },
                    );
                }
            }
            if max_width.is_some() {
                if let Ok(sign) = self.factory.CreateEllipsisTrimmingSign(&format) {
                    let trimming = DWRITE_TRIMMING {
                        granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                        delimiter: 0,
                        delimiterCount: 0,
                    };
                    let _ = layout.SetTrimming(&trimming, &sign);
                }
            }
        }
        Ok(layout)
    }
}

impl TextMetrics for TextEngine {
    fn measure(&self, text: &str, style: &TextStyle) -> Size {
        let key: MeasureKey = (
            text.to_string(),
            (style.font == Font::Mono, style.size.to_bits(), style.bold),
            style.tracking.to_bits(),
        );
        if let Some(size) = self.measured.borrow().get(&key) {
            return *size;
        }
        let size = self
            .layout(text, style, None)
            .map(|layout| {
                let mut metrics = DWRITE_TEXT_METRICS::default();
                // SAFETY: the output is a live local.
                let _ = unsafe { layout.GetMetrics(&mut metrics) };
                Size {
                    w: metrics.widthIncludingTrailingWhitespace,
                    h: metrics.height,
                }
            })
            // Without the font, an estimate: never zero, so a layout still works.
            .unwrap_or(Size {
                w: text.chars().count() as f32 * style.size * 0.6,
                h: style.size * 1.3,
            });
        let mut cache = self.measured.borrow_mut();
        if cache.len() >= MEASURES_KEPT {
            cache.clear();
        }
        cache.insert(key, size);
        size
    }
}
