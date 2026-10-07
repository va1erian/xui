# xui-blitz

A web view for xui whose pages are parsed, styled, laid out and painted by
[Blitz](https://github.com/DioxusLabs/blitz): html5ever, Servo's Stylo (the
Firefox style system), Taffy (block, flex, grid, floats) and Parley (text). It
replaces both earlier views: `xui-litehtml`'s `HtmlView` (mail and help
readers) and `xui-netsurf`'s `NetSurfView` (the LazyWeb browser).

Blitz is a dependency from crates.io, pinned to `=0.3.0-beta.2`; it is not
forked or patched. What it lacks that pages need is filled in from this side
(below).

## Using it

```rust
// A mail or help reader: links come back to the app.
let view = BlitzView::builder(|| Msg::Frame, |e| Some(Msg::Web(e)))
    .html(message_html)
    .base_url("cid:")
    .background(theme.background)
    .build(ui, bounds)?;

// A browser: the view follows links and forms itself.
xui_blitz::set_fetcher(Arc::new(MyHttpClient));
xui_blitz::set_downloader(Arc::new(MySaver));
let view = BlitzView::builder(|| Msg::Frame, |e| Some(Msg::Web(e)))
    .url("https://example.com/")
    .follow_links(true)
    .build(ui, bounds)?;

// In update(): Msg::Frame => view.update(),
```

`on_frame`'s message says a new frame is ready; answer it with `update()`.
`on_event` maps `BlitzViewEvent`s: `TitleChanged`, `UrlChanged`,
`LoadingChanged`, `StatusChanged` (the link under the pointer), `LinkClicked`
(when not following links), `LaunchUrl` (`mailto:` and other schemes the view
cannot open), `CopyRequested` (Ctrl+C on a selection), `FetchFailed`,
`Failed` and the `Download*` events. The view has `load_html`, `navigate`,
`stop`, `download`, `cancel_download`, `set_scroll`, `clear_selection`,
`set_background`, `is_ready` and `has_failed`.

### Moving from `xui-litehtml`

`HtmlView::new(ui, bounds, html, on_frame, on_event)` becomes
`BlitzView::builder(on_frame, on_event).html(html).build(ui, bounds)`;
`load(html)` becomes `load_html(html, base_url)`, and the frame message calls
`update()` instead of `invalidate()`. `LinkClicked` and `CopyRequested` are
the same events. `set_image_fetcher` is gone: images load through the
`Fetcher` like everything else (and `data:` and `file:` ones need nothing).

### Moving from `xui-netsurf`

The `Fetcher`, `FetchRequest`, `FetchResponder`, `Downloader`, `DownloadSink`,
`DownloadInfo` and `DownloadId` types have the same shape, so an existing
fetcher and downloader work by changing the import. Two differences: the view
keeps no cookies (a fetcher that wants them keeps a jar), and
`DownloadInfo::filename` already has `:`, path separators and the other
characters Windows refuses replaced. Blitz draws from font
files, not the backend's shaper: with no system fonts to find (LazyOS),
register them with `register_font(bytes)` and name the generic families with
`set_font_families`. Events arrive
through `on_event` instead of `update()`'s return value.

## Design

- One **engine thread per view** (`src/engine/`) owns the Blitz document;
  Blitz keeps no global state. The UI side (`src/widget.rs`) sends it size,
  scale, theme and input and paints the newest frame.
- The engine restyles and relays out, then draws the visible part of the page
  with the `vello_cpu` rasteriser into an RGBA `Image`, which the view blits
  with `Canvas::draw_image`. Blitz draws its own glyphs, so text looks the
  same on every backend and does not use `Ui::text_shaper`.
- `src/net/` loads everything: `file:`, `data:` and `about:blank` itself,
  `http(s):` through the application's `Fetcher`, following redirects. A page
  response the view does not display (or a `Content-Disposition: attachment`)
  is streamed to the `Downloader` instead.
- The xui theme's light or dark becomes the page's `prefers-color-scheme`.

### What is filled in around Blitz

- **XHTML doctypes.** Blitz parses anything that starts with an XHTML doctype
  as XML, which leaves most HTML mail unstyled; `text/html` is always parsed
  as HTML here (`engine/page.rs`).
- **Legacy presentational attributes.** Body `text`/`link`/`background`,
  `<font color size face>` and table `cellpadding`/`cellspacing`/`border`
  become a per-document user-agent style sheet (`engine/hints.rs`).
- **Keyboard scrolling** (arrows, Page Up/Down, Home/End) when no field has
  the focus.

## Fonts

With `system-fonts` (default) the platform's fonts are found: DirectWrite,
CoreText, or fontconfig loaded at run time (`dlopen`, so building needs no
fontconfig headers). A static binary has no fontconfig to load, so a system
like LazyOS builds with `default-features = false` and registers the font
files it ships:

```rust
xui_blitz::register_font(LIBERATION_SANS.to_vec());
xui_blitz::set_font_families(FontFamilies {
    sans_serif: "Liberation Sans".into(),
    ..FontFamilies::default()
});
```

Give each generic family a face with a real italic: Blitz's synthesized
italic leans backwards (upstream).

## Licence

MIT. Blitz and its dependencies are MIT or Apache-2.0, except Stylo
(MPL-2.0, file-level copyleft). No C, and no `unsafe` in this crate.

## Building for LazyOS

`xui-blitz` is pure Rust: with `default-features = false` it builds for
`x86_64-unknown-linux-musl` as a static-pie binary linked by `rust-lld` alone,
the way LazyOS's `tools/xui/build.py` links on Windows (no C compiler, no
zig). CI checks that build.

## Comparing engines

`scripts/engine-compare/` renders fixtures with litehtml, NetSurf, Blitz and
Chrome (offline) and measures each engine's cost in binary size: see
`scripts/engine-compare/README.md`.

## Not yet

JavaScript (Blitz has none), cookies, a disk cache, HTTP authentication,
`<iframe>`s (untested), the body's background image
covering the whole viewport, `vlink` (there is no history), IME composition
(typed characters work), and accessibility (Blitz builds an AccessKit tree
that is not wired to xui's yet).
