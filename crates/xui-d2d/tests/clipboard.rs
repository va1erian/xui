#![cfg(windows)]

//! The system clipboard through the backend. It replaces the real clipboard's
//! text, so run it in the sandbox (`scripts\sandbox\run.ps1`).

use std::sync::{Mutex, MutexGuard};

use xui_core::backend::Backend;
use xui_d2d::D2dBackend;

/// The clipboard is one per desktop, so the tests that use it take turns.
static CLIPBOARD: Mutex<()> = Mutex::new(());

fn exclusive() -> MutexGuard<'static, ()> {
    CLIPBOARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn text_round_trips_through_the_clipboard() {
    let _turn = exclusive();
    let backend = D2dBackend::new();
    backend.set_clipboard_text("hello, clipboard");
    assert_eq!(
        backend.clipboard_text().as_deref(),
        Some("hello, clipboard")
    );
}

#[test]
fn characters_outside_the_basic_plane_and_line_breaks_survive() {
    let _turn = exclusive();
    let backend = D2dBackend::new();
    let text = "é 𝄞 日本語\r\nsecond line";
    backend.set_clipboard_text(text);
    assert_eq!(backend.clipboard_text().as_deref(), Some(text));
}

#[test]
fn setting_again_replaces_the_text() {
    let _turn = exclusive();
    let backend = D2dBackend::new();
    backend.set_clipboard_text("first");
    backend.set_clipboard_text("second");
    assert_eq!(backend.clipboard_text().as_deref(), Some("second"));
}

#[test]
fn threads_using_the_clipboard_together_never_read_torn_text() {
    let _turn = exclusive();
    let markers: Vec<String> = (0..4).map(|n| format!("thread {n} ").repeat(200)).collect();
    let workers: Vec<_> = markers
        .iter()
        .cloned()
        .map(|mine| {
            let all = markers.clone();
            std::thread::spawn(move || {
                let backend = D2dBackend::new();
                for _ in 0..100 {
                    backend.set_clipboard_text(&mine);
                    if let Some(read) = backend.clipboard_text() {
                        assert!(
                            all.contains(&read),
                            "torn or foreign text: {} bytes",
                            read.len()
                        );
                    }
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
}
