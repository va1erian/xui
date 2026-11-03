//! Embeds the Win32 backend's application manifest (Common Controls v6 and
//! per-monitor-v2 DPI) into this crate's examples, so a gallery built here
//! shows the modern themed controls. Resources only make sense for an
//! executable, so the library itself is left untouched; a no-op elsewhere.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    println!("cargo:rerun-if-changed=xui.rc");
    println!("cargo:rerun-if-changed=../xui-win32/win32ui.manifest");
    embed_resource::compile_for_examples("xui.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("compile the xui example manifest");
}
