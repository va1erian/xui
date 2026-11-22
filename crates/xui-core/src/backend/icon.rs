#![forbid(unsafe_code)]

//! [`WindowIcon`]: what a top-level window shows in its title bar, the task
//! bar and the window switcher.

use crate::image::Image;

/// A top-level window's icon.
///
/// Set one for the app with [`Launch::icon`](crate::Launch::icon) (windows the
/// app opens later inherit it), for one window with
/// [`PlatformSpec::icon`](super::PlatformSpec::icon), or at run time with
/// [`Ui::set_window_icon`](crate::Ui::set_window_icon).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum WindowIcon {
    /// The same picture as RGBA images at one or more sizes. A backend picks
    /// the image that suits each size it needs (see [`WindowIcon::closest`])
    /// and resamples it when no image has that exact size.
    Images(Vec<Image>),
    /// An icon resource embedded in the executable, by its integer id (a
    /// Windows `RT_GROUP_ICON`). A Windows backend loads it at the sizes the
    /// window's DPI calls for; a backend with no executable resources ignores
    /// it.
    Resource(u16),
}

impl WindowIcon {
    /// An icon drawn from one image.
    pub fn image(image: Image) -> WindowIcon {
        WindowIcon::Images(vec![image])
    }

    /// An icon drawn from the same picture at several sizes.
    pub fn images(images: impl IntoIterator<Item = Image>) -> WindowIcon {
        WindowIcon::Images(images.into_iter().collect())
    }

    /// The executable's icon resource `id`.
    pub fn resource(id: u16) -> WindowIcon {
        WindowIcon::Resource(id)
    }

    /// The image best suited to a square icon of `size` pixels: the smallest
    /// one at least that large on its longer side, or the largest one when all
    /// are smaller. `None` for a resource icon or an empty list.
    pub fn closest(&self, size: u32) -> Option<&Image> {
        let WindowIcon::Images(images) = self else {
            return None;
        };
        let side = |image: &&Image| image.width().max(image.height());
        images
            .iter()
            .filter(|image| side(image) >= size)
            .min_by_key(side)
            .or_else(|| images.iter().max_by_key(side))
    }
}

impl From<Image> for WindowIcon {
    fn from(image: Image) -> WindowIcon {
        WindowIcon::image(image)
    }
}

#[cfg(test)]
mod tests {
    use super::WindowIcon;
    use crate::image::Image;

    fn square(side: u32) -> Image {
        Image::from_rgba(side, side, vec![0xFF; (side * side * 4) as usize]).expect("image")
    }

    #[test]
    fn closest_picks_the_smallest_image_that_is_large_enough() {
        let icon = WindowIcon::images([square(48), square(16), square(32)]);
        assert_eq!(icon.closest(16).map(Image::width), Some(16));
        assert_eq!(icon.closest(20).map(Image::width), Some(32));
        assert_eq!(icon.closest(40).map(Image::width), Some(48));
    }

    #[test]
    fn closest_falls_back_to_the_largest_image() {
        let icon = WindowIcon::images([square(16), square(32)]);
        assert_eq!(icon.closest(64).map(Image::width), Some(32));
    }

    #[test]
    fn a_resource_or_an_empty_list_has_no_image() {
        assert!(WindowIcon::resource(1).closest(32).is_none());
        assert!(WindowIcon::images([]).closest(32).is_none());
    }

    #[test]
    fn an_image_converts_into_a_one_image_icon() {
        let image = square(8);
        assert_eq!(
            WindowIcon::from(image.clone()),
            WindowIcon::Images(vec![image])
        );
    }
}
