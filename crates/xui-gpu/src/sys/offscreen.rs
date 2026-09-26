//! An offscreen colour+depth framebuffer for rendering a GL widget into a
//! texture and reading it back as pixels.

use glow::HasContext;

use xui_core::color::Color;

/// Whether the current context supports framebuffer objects, which an offscreen
/// frame needs.
///
/// The `winit` canvas backend can end up on a legacy OpenGL 1.1 context (the
/// Microsoft software renderer on a machine with no GL driver, as on a CI
/// runner), where `glGenFramebuffers` is not loaded; `glow` would panic on it.
/// Reading `GL_VERSION` (an OpenGL 1.0 entry point) lets the caller fall back to
/// the widget's software paint instead.
pub(crate) fn supports_framebuffers(gl: &glow::Context) -> bool {
    // SAFETY: `glGetString(GL_VERSION)` exists in every OpenGL version and the
    // context is current.
    let version = unsafe { gl.get_parameter_string(glow::VERSION) };
    gl_major(&version) >= 3
}

/// The major version at the start of a `GL_VERSION` string, or `0` when it does
/// not begin with one.
fn gl_major(version: &str) -> u32 {
    version
        .split('.')
        .next()
        .and_then(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .ok()
        })
        .unwrap_or(0)
}

/// A colour texture with a depth/stencil renderbuffer, sized to a widget.
pub(crate) struct Offscreen {
    framebuffer: glow::Framebuffer,
    color: glow::Texture,
    depth: glow::Renderbuffer,
    size: (u32, u32),
}

impl Offscreen {
    /// Builds a complete framebuffer of `width` x `height`, or `None` (deleting
    /// whatever it created) when a name cannot be created or the framebuffer is
    /// incomplete. The context must be current.
    pub(crate) fn new(gl: &glow::Context, width: u32, height: u32) -> Option<Offscreen> {
        let (width, height) = (width.max(1), height.max(1));
        // SAFETY: a context is current; every handle is created on it and any
        // partial result is deleted before returning. The attachment sizes
        // match, and completeness is checked before the framebuffer is used.
        unsafe {
            let framebuffer = match gl.create_framebuffer() {
                Ok(framebuffer) => framebuffer,
                Err(_) => return None,
            };
            let color = match gl.create_texture() {
                Ok(color) => color,
                Err(_) => {
                    gl.delete_framebuffer(framebuffer);
                    return None;
                }
            };
            gl.bind_texture(glow::TEXTURE_2D, Some(color));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(color),
                0,
            );
            let depth = match gl.create_renderbuffer() {
                Ok(depth) => depth,
                Err(_) => {
                    gl.bind_framebuffer(glow::FRAMEBUFFER, None);
                    gl.delete_texture(color);
                    gl.delete_framebuffer(framebuffer);
                    return None;
                }
            };
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(depth));
            gl.renderbuffer_storage(
                glow::RENDERBUFFER,
                glow::DEPTH24_STENCIL8,
                width as i32,
                height as i32,
            );
            gl.framebuffer_renderbuffer(
                glow::FRAMEBUFFER,
                glow::DEPTH_STENCIL_ATTACHMENT,
                glow::RENDERBUFFER,
                Some(depth),
            );
            let complete =
                gl.check_framebuffer_status(glow::FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE;
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_renderbuffer(glow::RENDERBUFFER, None);
            if !complete {
                gl.delete_renderbuffer(depth);
                gl.delete_texture(color);
                gl.delete_framebuffer(framebuffer);
                return None;
            }
            Some(Offscreen {
                framebuffer,
                color,
                depth,
                size: (width, height),
            })
        }
    }

    /// The framebuffer size in pixels.
    pub(crate) fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Binds the framebuffer, sets the viewport to it and clears colour and
    /// depth to `background`.
    pub(crate) fn begin(&self, gl: &glow::Context, width: u32, height: u32, background: Color) {
        // SAFETY: a context is current and `self` owns a complete framebuffer.
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.framebuffer));
            gl.viewport(0, 0, width.max(1) as i32, height.max(1) as i32);
            gl.clear_color(
                f32::from(background.r) / 255.0,
                f32::from(background.g) / 255.0,
                f32::from(background.b) / 255.0,
                1.0,
            );
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        }
    }

    /// Reads the colour attachment back as top-down RGBA pixels. OpenGL's origin
    /// is bottom-left, so the rows are flipped into software (top-down) order.
    pub(crate) fn read(&self, gl: &glow::Context, width: u32, height: u32) -> Vec<u8> {
        let (width, height) = (width.max(1), height.max(1));
        let stride = (width * 4) as usize;
        let mut pixels = vec![0u8; stride * height as usize];
        // SAFETY: the framebuffer is complete and bound by `begin`, and `pixels`
        // is exactly `width * height * 4` bytes.
        unsafe {
            gl.read_pixels(
                0,
                0,
                width as i32,
                height as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
        }
        for y in 0..height as usize / 2 {
            let top = y * stride;
            let bottom = (height as usize - 1 - y) * stride;
            for i in 0..stride {
                pixels.swap(top + i, bottom + i);
            }
        }
        pixels
    }

    /// Restores the default framebuffer.
    pub(crate) fn unbind(gl: &glow::Context) {
        // SAFETY: a context is current; binding the default framebuffer is
        // always valid.
        unsafe { gl.bind_framebuffer(glow::FRAMEBUFFER, None) };
    }

    /// Deletes the framebuffer and its attachments. The context must be
    /// current and `self` must not be used afterwards.
    pub(crate) fn delete(self, gl: &glow::Context) {
        // SAFETY: a context is current and every handle was created on it.
        unsafe {
            gl.delete_renderbuffer(self.depth);
            gl.delete_texture(self.color);
            gl.delete_framebuffer(self.framebuffer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::gl_major;

    #[test]
    fn the_version_major_is_read_from_the_gl_version_string() {
        assert_eq!(gl_major("1.1.0"), 1);
        assert_eq!(gl_major("3.3.0 - Build 32.0.101.6314"), 3);
        assert_eq!(gl_major("4.6.0 NVIDIA 552.22"), 4);
        assert_eq!(gl_major("not a version"), 0);
        assert_eq!(gl_major(""), 0);
    }
}
