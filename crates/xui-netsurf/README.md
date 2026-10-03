# xui-netsurf (prototype)

A web view for xui whose pages are laid out by the
[NetSurf](https://www.netsurf-browser.org/) browser core, built to compare
against `xui-litehtml`. NetSurf's plotter calls are recorded into an
`xui-litehtml` display list and painted by the same `Painter`, with the same
text shaper, so the two engines differ only in layout.

## Licence

NetSurf is **GPL-2.0-only** (its libraries are MIT), so `xui-netsurf` and
`netsurf-sys` are GPL-2.0-only too. They are their own cargo workspace,
excluded from the root one: nothing in the MIT crates depends on them.

## Building

```bash
git submodule update --init --depth 1 crates/xui-netsurf/netsurf-sys/vendor
cd crates/xui-netsurf
cargo run --example compare                         # a window, the built-in page
cargo run --example compare -- page.html            # any local file
cargo run --example compare -- --screenshot out.png # offscreen, no display needed
cargo test --workspace
```

`netsurf-sys` compiles NetSurf's core and its libraries (libcss, libdom,
libhubbub, libparserutils, libwapcaplet, libnsutils, libnsgif, libnsbmp) with
the `cc` crate; it needs a C compiler and nothing else (zlib comes from
`libz-sys`). The C the libraries generate at build time is checked in under
`netsurf-sys/generated/`; `netsurf-sys/scripts/regen.sh` remakes it after a
submodule pin moves. So far it is built and tested on Linux only.

## Layout

| Path | What |
| --- | --- |
| `netsurf-sys/csrc/nsx.h` | The flat C interface over NetSurf's frontend tables |
| `netsurf-sys/csrc/nsx_*.c` | Scheduler, window, plotter and bitmap glue |
| `src/engine.rs` | The one engine thread (NetSurf's core is global) |
| `src/fonts.rs` | Text measuring for NetSurf's layout, over xui's shaper |
| `src/record.rs` | Plotter calls into an `xui-litehtml` display list |
| `src/sys/` | The only `unsafe`: calls into C and the C callbacks |
| `src/view.rs`, `src/widget/` | `NetSurfView`, a custom-painted node |
| `examples/compare/` | Both engines side by side |

## What works and what does not

Works: HTML and CSS 2.1 layout, flexbox, floats, tables, form controls
(drawn by NetSurf), link clicks and typing into fields (NetSurf handles
them), GIF and BMP images, `file:`, `data:`, `about:` and `resource:` URLs,
any DPI.

Not yet: network fetching (curl and TLS), PNG, JPEG, WebP and SVG images,
JavaScript (NetSurf's is Duktape, off here), text selection, the pointer
shape, horizontal scrolling, scrolling to fragment links, the Windows and
macOS builds, and the LazyOS (musl) cross build.
