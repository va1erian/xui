//! Gives the test binaries an icon resource, so the tests can check that a
//! window shows the executable's icon with no app code.
//!
//! The library itself gets nothing. The `.res` is written by hand rather than
//! by a resource compiler, so building needs no Windows SDK tool, and it is
//! linked only into tests (`rustc-link-arg-tests`) on MSVC, whose linker
//! accepts a `.res` file directly.

use std::path::PathBuf;

/// The id of the icon group the tests load; the tests mirror it.
const GROUP_ID: u16 = 1;
/// `RT_ICON` and `RT_GROUP_ICON`, from winuser.h.
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
/// `MOVEABLE | PURE | DISCARDABLE`, the flags a resource compiler gives icons.
const ICON_FLAGS: u16 = 0x1030;
/// `MAKELANGID(LANG_ENGLISH, SUBLANG_ENGLISH_US)`.
const LANGUAGE: u16 = 0x0409;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !(windows && msvc) {
        return;
    }
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let path = out.join("test_icon.res");
    std::fs::write(&path, resource_file()).expect("the test icon resource");
    println!("cargo:rustc-link-arg-tests={}", path.display());
}

/// A `.res` file holding one icon group with a 16 and a 32 pixel image.
fn resource_file() -> Vec<u8> {
    let sizes = [(16u8, 1u16), (32, 2)];
    // A `.res` file starts with an empty entry that marks its format.
    let mut res = entry(0, 0, &[], 0);
    let mut group = Vec::new();
    for word in [0u16, 1, sizes.len() as u16] {
        group.extend_from_slice(&word.to_le_bytes());
    }
    for (side, id) in sizes {
        let image = icon_image(u32::from(side));
        res.extend(entry(RT_ICON, id, &image, ICON_FLAGS));
        // GRPICONDIRENTRY: width, height, colours, reserved, planes,
        // bits per pixel, image bytes, image id.
        group.extend_from_slice(&[side, side, 0, 0]);
        group.extend_from_slice(&1u16.to_le_bytes());
        group.extend_from_slice(&32u16.to_le_bytes());
        group.extend_from_slice(&(image.len() as u32).to_le_bytes());
        group.extend_from_slice(&id.to_le_bytes());
    }
    res.extend(entry(RT_GROUP_ICON, GROUP_ID, &group, ICON_FLAGS));
    res
}

/// One resource: its header (ordinal type and name), then its data padded to
/// a 4-byte boundary.
fn entry(kind: u16, name: u16, data: &[u8], flags: u16) -> Vec<u8> {
    const HEADER_SIZE: u32 = 32;
    let mut out = Vec::new();
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&HEADER_SIZE.to_le_bytes());
    for word in [0xFFFF, kind, 0xFFFF, name] {
        out.extend_from_slice(&u16::to_le_bytes(word));
    }
    out.extend_from_slice(&0u32.to_le_bytes()); // data version
    out.extend_from_slice(&flags.to_le_bytes());
    let language = if kind == 0 { 0 } else { LANGUAGE };
    out.extend_from_slice(&language.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // version
    out.extend_from_slice(&0u32.to_le_bytes()); // characteristics
    out.extend_from_slice(data);
    out.resize(out.len().next_multiple_of(4), 0);
    out
}

/// An opaque square icon image of `side` pixels as an `RT_ICON` stores it: a
/// `BITMAPINFOHEADER` of double height, bottom-up BGRA colour bits, then an
/// all-zero AND mask with rows padded to 32 bits.
fn icon_image(side: u32) -> Vec<u8> {
    let colour = (side * side * 4) as usize;
    let mask = (side.div_ceil(32) * 4 * side) as usize;
    let mut out = Vec::new();
    for value in [40u32, side, side * 2] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&((colour + mask) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 16]); // resolution and palette fields
    for _ in 0..side * side {
        out.extend_from_slice(&[0xD0, 0x60, 0x20, 0xFF]);
    }
    out.resize(out.len() + mask, 0);
    out
}
