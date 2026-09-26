//! A window-level GPU widget on the canvas backend: an animated, shaded
//! triangle drawn with `glow` OpenGL through [`GlWidget::paint_gl`].
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui-canvas --example gl
//! ```
//!
//! Setting `XUI_DEMO_AUTOCLOSE_MS` makes it quit itself, for a headless smoke
//! run. When no GL context can be created the triangle is drawn with the
//! software fallback, so the window still shows something.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use xui_canvas::glow::{self, HasContext};
use xui_canvas::{GlWidget, WinitBackend};
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::{Dip, Point, Rect, Theme};

const VERTEX: &str = r#"
#version 330 core
layout (location = 0) in vec2 a_pos;
layout (location = 1) in vec3 a_color;
uniform float u_time;
out vec3 v_color;
void main() {
    float s = sin(u_time);
    float c = cos(u_time);
    vec2 p = vec2(a_pos.x * c - a_pos.y * s, a_pos.x * s + a_pos.y * c);
    v_color = a_color;
    gl_Position = vec4(p, 0.0, 1.0);
}
"#;

const FRAGMENT: &str = r#"
#version 330 core
in vec3 v_color;
out vec4 frag_color;
void main() {
    frag_color = vec4(v_color, 1.0);
}
"#;

/// Position (`x, y`) followed by colour (`r, g, b`) per vertex.
const TRIANGLE: [f32; 15] = [
    0.0, 0.7, 1.0, 0.35, 0.35, //
    -0.7, -0.6, 0.35, 1.0, 0.35, //
    0.7, -0.6, 0.35, 0.35, 1.0,
];

/// The GPU objects built once per context, freed in `gl_teardown`.
struct Resources {
    program: glow::Program,
    vao: glow::VertexArray,
    time: Option<glow::UniformLocation>,
}

/// A rotating, vertex-coloured triangle.
struct Triangle {
    angle: Cell<f32>,
    resources: RefCell<Option<Resources>>,
    failed: Cell<bool>,
}

impl Triangle {
    fn new() -> Triangle {
        Triangle {
            angle: Cell::new(0.0),
            resources: RefCell::new(None),
            failed: Cell::new(false),
        }
    }

    fn build(gl: &glow::Context) -> Result<Resources, String> {
        // SAFETY: `gl` is current because a surface frame is active.
        unsafe {
            let program = gl.create_program()?;
            let mut shaders = Vec::new();
            for (kind, source) in [
                (glow::VERTEX_SHADER, VERTEX),
                (glow::FRAGMENT_SHADER, FRAGMENT),
            ] {
                let shader = gl.create_shader(kind)?;
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    return Err(gl.get_shader_info_log(shader));
                }
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(gl.get_program_info_log(program));
            }
            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }

            let vao = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(vao));
            let vbo = gl.create_buffer()?;
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            let bytes =
                std::slice::from_raw_parts(TRIANGLE.as_ptr() as *const u8, TRIANGLE.len() * 4);
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STATIC_DRAW);
            let stride = 5 * std::mem::size_of::<f32>() as i32;
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 2 * 4);

            Ok(Resources {
                program,
                vao,
                time: gl.get_uniform_location(program, "u_time"),
            })
        }
    }
}

impl GlWidget for Triangle {
    fn paint(&self, canvas: &mut dyn xui_core::Canvas, bounds: Rect, theme: &Theme) {
        canvas.clear(theme.background);
        let mid = Point::new(bounds.width() / 2, bounds.height() / 2);
        let r = (bounds.width().min(bounds.height()) as f32) * 0.35;
        let points = [
            Point::new(mid.x, mid.y - r as i32),
            Point::new(mid.x - r as i32, mid.y + (r * 0.5) as i32),
            Point::new(mid.x + r as i32, mid.y + (r * 0.5) as i32),
        ];
        canvas.fill_polygon(&points, theme.accent);
    }

    fn paint_gl(&self, gl: &glow::Context, _bounds: Rect, _theme: &Theme) {
        let mut resources = self.resources.borrow_mut();
        if resources.is_none() {
            if self.failed.get() {
                return;
            }
            match Triangle::build(gl) {
                Ok(built) => *resources = Some(built),
                Err(error) => {
                    self.failed.set(true);
                    eprintln!("gl example: shader build failed: {error}");
                    return;
                }
            }
        }
        let resources = resources.as_ref().expect("built above");
        // SAFETY: a context is current and every handle belongs to it.
        unsafe {
            gl.use_program(Some(resources.program));
            gl.bind_vertex_array(Some(resources.vao));
            gl.uniform_1_f32(resources.time.as_ref(), self.angle.get());
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
        }
    }

    fn gl_teardown(&self, gl: &glow::Context) {
        if let Some(resources) = self.resources.borrow_mut().take() {
            // SAFETY: the surface made its context current for this teardown.
            unsafe {
                gl.delete_program(resources.program);
                gl.delete_vertex_array(resources.vao);
            }
        }
    }
}

struct Demo;

impl App for Demo {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

fn main() {
    let backend = Rc::new(WinitBackend::new());
    let time = Rc::new(Cell::new(0.0f32));
    let time_for_make = Rc::clone(&time);
    let backend_for_make = Rc::clone(&backend);
    let backend_for_timer = Rc::clone(&backend);

    let dynamic: Rc<dyn Backend> = backend;
    let _ = run_app(
        dynamic,
        PlatformSpec::new("xui-canvas GL visualizer").size(Dip(520.0), Dip(420.0)),
        move |ui| {
            let window = ui.window();
            let triangle = Triangle::new();
            backend_for_make.set_gl_content(window, triangle);

            let anim = ui.set_timer(16);
            let autoclose = std::env::var("XUI_DEMO_AUTOCLOSE_MS")
                .ok()
                .and_then(|millis| millis.parse::<u32>().ok())
                .map(|millis| ui.set_timer(millis));
            ui.on_timer(move |fired| {
                if fired == anim {
                    let next = time_for_make.get() + 0.02;
                    time_for_make.set(if next > std::f32::consts::TAU {
                        next - std::f32::consts::TAU
                    } else {
                        next
                    });
                    backend_for_timer.request_redraw(window);
                } else if Some(fired) == autoclose {
                    backend_for_timer.quit(0);
                }
                None
            });
            Demo
        },
    );
}
