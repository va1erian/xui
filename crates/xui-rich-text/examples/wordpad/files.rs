#![forbid(unsafe_code)]

//! The wordpad's file I/O: the library does none, so it lives here.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use xui_core::Dip;
use xui_core::image::Image;
use xui_rich_text::format::{ImageExport, from_json, to_json, to_markdown};
use xui_rich_text::model::{Document, InlineImage, Wrap};

/// The widest an inserted image gets, in design units.
const MAX_IMAGE_WIDTH: f32 = 360.0;

/// Writes `doc` as the native JSON format.
pub fn save_json(path: &Path, doc: &Document) -> Result<(), String> {
    std::fs::write(path, to_json(doc)).map_err(|e| e.to_string())
}

/// Reads a document saved by [`save_json`].
pub fn load_json(path: &Path) -> Result<Document, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    from_json(&text).map_err(|e| e.to_string())
}

/// Decodes an image file into an inline image no wider than a page column.
pub fn load_image(path: &Path) -> Result<InlineImage, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let image = Image::decode(&bytes).map_err(|e| e.to_string())?;
    let (w, h) = (image.width() as f32, image.height() as f32);
    let scale = (MAX_IMAGE_WIDTH / w).min(1.0);
    Ok(InlineImage {
        image: Arc::new(image),
        size: (Dip(w * scale), Dip(h * scale)),
        wrap: Wrap::Inline,
        alt: path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    })
}

/// Writes `doc` as Markdown to `path`, saving its images as PNGs in
/// `<name>_images/` beside it and linking them relatively.
pub fn export_markdown(path: &Path, doc: &Document) -> Result<(), String> {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".to_owned());
    let folder = format!("{stem}_images");
    let dir: PathBuf = path.parent().unwrap_or(Path::new(".")).join(&folder);
    let count = Rc::new(Cell::new(0usize));
    let failure: Rc<RefCell<Option<String>>> = Rc::default();

    let (counter, error) = (Rc::clone(&count), Rc::clone(&failure));
    let export = ImageExport::Callback(Box::new(move |image, _alt| {
        counter.set(counter.get() + 1);
        let name = format!("{}.png", counter.get());
        let written = std::fs::create_dir_all(&dir)
            .map_err(|e| e.to_string())
            .and_then(|()| image.save_png(dir.join(&name)).map_err(|e| e.to_string()));
        if let Err(e) = written {
            error.borrow_mut().get_or_insert(e);
        }
        format!("{folder}/{name}")
    }));
    let markdown = to_markdown(doc, &export);
    if let Some(error) = failure.borrow_mut().take() {
        return Err(error);
    }
    std::fs::write(path, markdown).map_err(|e| e.to_string())
}
