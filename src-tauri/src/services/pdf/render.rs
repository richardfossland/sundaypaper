//! pdfium-backed rasterised rendering + text extraction. Needs the pdfium
//! dynamic library present at runtime; compiled only under the `pdf` feature.
//!
//! Unlike `edit` (pure lopdf, round-trip tested), these paths cannot be
//! exercised in CI — there is no pdfium binary in the build env — so they carry
//! no runtime tests. Their signatures are still compile-checked under
//! `--features pdf`, and a missing library surfaces as a clear `Pdf` error
//! rather than a panic.

use std::io::Cursor;
use std::path::Path;

use pdfium_render::prelude::*;

use crate::error::{AppError, AppResult};

fn pdf_err<E: std::fmt::Display>(e: E) -> AppError {
    AppError::Pdf(e.to_string())
}

fn pdfium() -> AppResult<Pdfium> {
    let bindings = Pdfium::bind_to_system_library()
        .map_err(|e| AppError::Pdf(format!("could not load the pdfium library: {e}")))?;
    Ok(Pdfium::new(bindings))
}

fn path_str(path: &Path) -> AppResult<&str> {
    path.to_str()
        .ok_or_else(|| AppError::Pdf("path is not valid UTF-8".into()))
}

/// Extract document text, one page per entry joined by form-feed (`\f`) so the
/// caller can split per page.
pub fn extract_text(path: &Path) -> AppResult<String> {
    let pdfium = pdfium()?;
    let document = pdfium
        .load_pdf_from_file(path_str(path)?, None)
        .map_err(pdf_err)?;
    let mut pages_text = Vec::new();
    for page in document.pages().iter() {
        let text = page.text().map_err(pdf_err)?;
        pages_text.push(text.all());
    }
    Ok(pages_text.join("\u{000C}"))
}

/// Render one page (0-based `index`) to PNG bytes, scaled to `target_width`
/// pixels (height follows the page aspect ratio).
pub fn render_page_png(path: &Path, index: u16, target_width: u32) -> AppResult<Vec<u8>> {
    let pdfium = pdfium()?;
    let document = pdfium
        .load_pdf_from_file(path_str(path)?, None)
        .map_err(pdf_err)?;
    let page = document.pages().get(index.into()).map_err(pdf_err)?;
    let config = PdfRenderConfig::new().set_target_width(target_width as i32);
    let bitmap = page.render_with_config(&config).map_err(pdf_err)?;
    let mut bytes = Vec::new();
    bitmap
        .as_image()
        .map_err(pdf_err)?
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .map_err(pdf_err)?;
    Ok(bytes)
}
