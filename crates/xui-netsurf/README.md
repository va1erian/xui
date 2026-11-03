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
cd crates/xui-netsurf
cargo run --example compare                         # a window, the built-in page
cargo run --example compare -- page.html            # any local file
cargo run --example compare -- --screenshot out.png # offscreen, no display needed
cargo test --workspace
```

`netsurf-sys` compiles NetSurf's core and its libraries (libcss, libdom,
libhubbub, libparserutils, libwapcaplet, libnsutils, libnsgif, libnsbmp) with
the `cc` crate; it needs a C compiler and nothing else (zlib comes from
`libz-sys`, built from source). There is no curl, OpenSSL, libpng, libjpeg or
libsvgtiny: the network and the PNG, JPEG and SVG decoders are Rust (below). The NetSurf sources are vendored
in `netsurf-sys/vendor/`, pruned to what the build compiles (no upstream test
suites, whose fuzzer-named files cannot be checked out on Windows), from the
commits in `netsurf-sys/scripts/revisions`; `netsurf-sys/scripts/vendor.sh`
refreshes them and applies `netsurf-sys/patches/` (what nsx changes in
NetSurf: CSS `opacity`, the text of inline `::before`/`::after`, a space
after an inline element's opening tag, `display: none` for `<link>`,
`<meta>` and HTML's other hidden elements; refresh the patch with `git diff --relative=crates/xui-netsurf/netsurf-sys/vendor`
after editing `vendor/`). The C the libraries generate at build time is checked in under
`netsurf-sys/generated/`; `netsurf-sys/scripts/regen.sh` remakes it after a
pin moves. So far it is built and tested on Linux only.

## Fetching `http:` and `https:`

NetSurf has no HTTP client here. The application supplies one, any thread
safe type implementing `Fetcher`, before the first view opens:

```rust
use std::sync::Arc;
use xui_netsurf::{FetchRequest, FetchResponder, Fetcher, set_fetcher};

struct Http; // e.g. over ureq + rustls

impl Fetcher for Http {
    // Called on a thread of its own per request: blocking is fine.
    fn fetch(&self, request: FetchRequest, responder: FetchResponder) {
        // send request.method, request.url, request.headers, request.body;
        // do NOT follow redirects; decode Content-Encoding and drop it.
        responder.status(200);
        responder.header("Content-Type", "text/html");
        responder.data(b"<p>hello");
        responder.finish(); // or responder.fail("connection refused")
    }
}

set_fetcher(Arc::new(Http));
```

The fetcher is a transport, as libcurl is to NetSurf's own fetcher
(`netsurf-sys/csrc/nsx_fetch.c` stands in for `content/fetchers/curl.c`):
NetSurf follows `3xx` + `Location` itself, keeps cookies (`Set-Cookie` in,
`Cookie` out), sends its user agent and `Accept` headers, and encodes forms
(url-encoded or `multipart/form-data`) as the request body with its
`Content-Type`. Answers queue on the engine's command channel, so the engine
thread wakes for them; NetSurf aborts a request it no longer needs
(`FetchResponder::is_aborted`), and a responder dropped without `finish` or
`fail` fails the request. When the page a view is loading fails, the view
reports `NetSurfViewEvent::FetchFailed { url, message }` and NetSurf shows its
error page.

## A browser around the view

`NetSurfView` reports what a browser's chrome shows and does:

- `StatusChanged(text)`: NetSurf's status line, the link under the pointer or
  the load's progress (`Fetching`, `Processing`, `Done (0.3s)`).
- `LaunchUrl { url, by_user }`: a link NetSurf has no fetcher for
  (`mailto:`), for the application to hand to the system. `by_user` is false
  when no click, key or `navigate` asked for it (a page's meta refresh or
  script), so an application can refuse to start another program for a page
  on its own.
- Downloads: a response NetSurf cannot show (an archive, a PDF, a
  `Content-Disposition: attachment`) or a `NetSurfView::download(url)` is
  offered to the `Downloader` set with `set_downloader`, whose `DownloadSink`
  takes the bytes on the engine thread; the view reports `DownloadStarted`,
  `DownloadProgress` and `DownloadFinished`, and `cancel_download` stops one.
- `NetSurfView::stop` stops a load.

## Layout

| Path | What |
| --- | --- |
| `netsurf-sys/csrc/nsx.h` | The flat C interface over NetSurf's frontend tables |
| `netsurf-sys/csrc/nsx_*.c` | Scheduler, window, plotter and bitmap glue |
| `netsurf-sys/csrc/nsx_fetch.c`, `nsx_post.c` | The `http(s):` fetcher over the host's `Fetcher`, form bodies |
| `netsurf-sys/csrc/nsx_download.c` | NetSurf's download table, reported to the host by id |
| `netsurf-sys/csrc/nsx_image.c` | The PNG, JPEG and SVG content handler, decoded by the host |
| `netsurf-sys/patches/` | nsx's changes to the vendored NetSurf (opacity, inline generated text, inline spaces, hidden elements) |
| `src/fetch.rs` | The public `Fetcher` API and the fetches in flight |
| `src/download.rs` | The public `Downloader` API and the downloads under way |
| `src/image.rs` | PNG (`png`) and JPEG (`zune-jpeg`) decoding, capped at 16 megapixels |
| `src/svg.rs` | SVG (`resvg`, no text, nothing loaded from outside), drawn at 2x its size |
| `src/engine.rs` | The one engine thread (NetSurf's core is global) |
| `src/fonts.rs` | Text measuring for NetSurf's layout, over xui's shaper |
| `src/families.rs` | CSS font families sorted into the host's sans, serif and mono families (`set_font_families`) |
| `src/record.rs` | Plotter calls into an `xui-litehtml` display list |
| `src/sys/` | The only `unsafe`: calls into C and the C callbacks |
| `tests/fetch.rs` | A fake server: redirects, images, a 404, failures, cookies, a form POST |
| `tests/browser.rs` | Status text, a `mailto:` link, downloads (started, asked for, cancelled), Stop |
| `tests/render.rs` | Pixels: `opacity`, SVG pictures, inline `::after` text, spacing |
| `src/view.rs`, `src/widget/` | `NetSurfView`, a custom-painted node |
| `examples/compare/` | Both engines side by side |

## What works and what does not

Works: HTML and CSS 2.1 layout, flexbox, floats, tables, form controls
(drawn by NetSurf), link clicks, typing into fields and submitting forms
(GET and POST), `opacity` (per box, not composited as a group), the text of
inline `::before`/`::after` (strings and `attr()`), GIF, BMP, PNG, JPEG and
SVG images (SVG text is not drawn), `http:` and `https:` through
the application's `Fetcher` (redirects, cookies kept in memory), `file:`,
`data:`, `about:` and `resource:` URLs, any DPI. The nsx C glue compiles
for `x86_64-linux-musl` with `zig cc`.

Not yet: CSS grid, `var()` and `mask-image`, counters and quotes in
`content`, WebP and PNG-in-ICO images, animated PNG (the first frame
shows), HTTP authentication (a `401` shows the server's page), certificate
details and TLS error pages (the fetcher's `fail` message is shown instead),
a persistent cookie jar, a disc cache, JavaScript (NetSurf's is Duktape, off
here), text selection, the pointer shape, horizontal scrolling, scrolling to
fragment links, the Windows and macOS builds, and a tested LazyOS (musl) cross
build of the whole crate.
