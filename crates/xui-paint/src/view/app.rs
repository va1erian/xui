#![forbid(unsafe_code)]

//! The application: the window's layout of widgets and the `Msg` -> model
//! glue.

use std::cell::RefCell;
use std::rc::Rc;

use xui_core::app::{App, Ui};
use xui_core::arrange::{Handle, LayoutExt, build, column, status_bar};
use xui_core::backend::Result;
use xui_core::widget::StatusBar;

use super::canvas::CanvasMsg;
use super::layout::{Observer, Parts, strip_items};
use super::palette::Palette;
use super::toolbar::ToolStrip;
use super::{Msg, PaintCanvas};
use crate::model::{Model, Side};
use crate::storage::Storage;

use crate::{DEFAULT_HEIGHT, DEFAULT_WIDTH};

/// The paint application.
pub struct PaintApp {
    model: Model,
    canvas: Handle<PaintCanvas>,
    toolbar: Handle<ToolStrip>,
    palette: Handle<Palette>,
    status: Handle<StatusBar<Msg>>,
    storage: Rc<dyn Storage>,
    observer: Rc<RefCell<Observer>>,
    cursor: Option<(i32, i32)>,
    message: Option<String>,
}

impl PaintApp {
    /// Builds the widgets for `ui` and wires them to a fresh model.
    pub fn build(ui: &mut Ui<Msg>, storage: Rc<dyn Storage>) -> Result<PaintApp> {
        PaintApp::build_observed(ui, storage, Rc::new(RefCell::new(Observer::default())))
    }

    /// Like [`PaintApp::build`], additionally reporting state to `observer`.
    pub fn build_observed(
        ui: &mut Ui<Msg>,
        storage: Rc<dyn Storage>,
        observer: Rc<RefCell<Observer>>,
    ) -> Result<PaintApp> {
        let io = storage.available();
        let canvas = Handle::new();
        let toolbar = Handle::new();
        let palette = Handle::new();
        let status = Handle::new();
        ui.root(column().children((
            build(move |ui| ToolStrip::new(ui, strip_items(io))).bind(&toolbar),
            build(PaintCanvas::new).bind(&canvas).fill(1),
            build(Palette::new).bind(&palette),
            status_bar(&["--", "320 x 240", "Pencil"]).bind(&status),
        )))?;
        observer.borrow_mut().parts = Some(Parts {
            toolbar: toolbar.get().id(),
            canvas: canvas.get().id(),
            palette: palette.get().id(),
            status: status.get().id(),
        });

        let mut app = PaintApp {
            model: Model::new(DEFAULT_WIDTH, DEFAULT_HEIGHT),
            canvas,
            toolbar,
            palette,
            status,
            storage,
            observer,
            cursor: None,
            message: None,
        };
        app.sync();
        Ok(app)
    }

    /// The drawing model.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// The canvas widget.
    pub fn canvas(&self) -> Rc<PaintCanvas> {
        self.canvas.get()
    }

    /// The tool strip.
    pub fn toolbar(&self) -> Rc<ToolStrip> {
        self.toolbar.get()
    }

    /// The palette.
    pub fn palette(&self) -> Rc<Palette> {
        self.palette.get()
    }

    /// The last save/load message, if any.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Rebuilds the painter state, toolbar and status bar from the model.
    fn sync(&mut self) {
        self.canvas.get().sync(
            self.model.bitmap(),
            self.model.revision(),
            self.model.preview(),
            self.model.size(),
        );
        self.toolbar
            .get()
            .sync(&self.model, self.storage.available());
        self.palette
            .get()
            .sync(self.model.primary(), self.model.secondary());
        self.refresh_status();
    }

    /// Rewrites the status bar parts.
    fn refresh_status(&mut self) {
        let position = match self.cursor {
            Some((x, y)) => format!("{x}, {y}"),
            None => "--".to_string(),
        };
        let (width, height) = self.model.bitmap().size();
        let size = format!("{width} x {height}");
        let tool = self.model.tool().label().to_string();
        let message = self.message.clone().unwrap_or_default();
        let status = self.status.get();
        status.set_text(0, &position);
        status.set_text(1, &size);
        status.set_text(2, &tool);
        status.set_text(3, &message);

        let mut observer = self.observer.borrow_mut();
        observer.tool = self.model.tool();
        observer.size = self.model.size();
        observer.primary = self.model.primary();
        observer.secondary = self.model.secondary();
        observer.can_undo = self.model.history().can_undo();
        observer.can_redo = self.model.history().can_redo();
        observer.cursor = self.cursor;
        observer.dragging = self.model.is_dragging();
        observer.status = [position, size, tool, message];
    }

    /// Saves the bitmap through the storage, reporting the outcome.
    fn save(&mut self) {
        let result = self
            .model
            .bitmap()
            .encode_png()
            .map_err(|error| error.to_string())
            .and_then(|bytes| self.storage.save(&bytes));
        self.message = Some(match result {
            Ok(()) => "Saved".to_string(),
            Err(error) => format!("Save failed: {error}"),
        });
    }

    /// Decodes first, then swaps, so a failed load leaves the canvas intact.
    fn open(&mut self) {
        let Some(bytes) = self.storage.load() else {
            self.message = Some("Open failed: nothing stored".to_string());
            return;
        };
        match crate::model::Bitmap::decode(&bytes) {
            Ok(bitmap) => {
                self.model.load(bitmap);
                self.cursor = None;
                self.message = Some("Opened".to_string());
            }
            Err(error) => self.message = Some(format!("Open failed: {error}")),
        }
    }
}

impl App for PaintApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Tool(tool) => self.model.set_tool(tool),
            Msg::Size(size) => self.model.set_size(size),
            Msg::Palette { color, side } => match side {
                Side::Primary => self.model.set_primary(color),
                Side::Secondary => self.model.set_secondary(color),
            },
            Msg::SwapColors => self.model.swap_colors(),
            Msg::Undo => self.model.undo(),
            Msg::Redo => self.model.redo(),
            Msg::Clear => self.model.clear(),
            Msg::New => {
                self.model.reset(DEFAULT_WIDTH, DEFAULT_HEIGHT);
                self.cursor = None;
            }
            Msg::Save => self.save(),
            Msg::Open => self.open(),
            Msg::Canvas(CanvasMsg::Down { x, y, side }) => {
                if self.model.is_dragging() {
                    self.model.end();
                }
                self.cursor = Some((x, y));
                self.model.begin(x, y, side);
            }
            Msg::Canvas(CanvasMsg::Move { x, y }) => {
                self.cursor = Some((x, y));
                self.model.extend(x, y);
            }
            Msg::Canvas(CanvasMsg::Up { x, y }) => {
                self.cursor = Some((x, y));
                self.model.extend(x, y);
                self.model.end();
            }
            Msg::Canvas(CanvasMsg::Cancel) => {
                self.model.cancel();
                self.cursor = None;
            }
        }
        self.sync();
    }
}
