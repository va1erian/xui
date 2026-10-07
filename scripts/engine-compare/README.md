# Engine comparison

Renders the same pages with xui's three HTML engines (litehtml, NetSurf and
Blitz) and with Chrome, and measures what each engine adds to a binary.

## Pieces

| | |
|---|---|
| `probes/` | A cargo workspace of four headless programs with one skeleton (`common.rs`): `baseline` (no engine) and one per engine. `probe-<engine> page.html out.png [width] [height]` renders the page offscreen at 96 dpi once it has loaded and prints `PROBE:<engine>:ready=..:ms=..`. Its own workspace: the NetSurf probe links GPL-2.0-only C that builds only on Linux. |
| `fixtures/` | Committed pages: a CSS feature page, the NetSurf comparison demo, example.com, a table-and-`<font>` newsletter in the shape of HTML mail, and theoldnet (legacy attributes, tiled background, images). |
| `fetch_live.py` | Saves Wikipedia's *Rust (programming language)* and the Hacker News front page with their style sheets inlined and scripts removed, under `target/engine-compare/live/` (not committed). |
| `run.sh` | Linux (or WSL): builds the probes in the `release` and `small` (`opt-level = "s"`, fat LTO, `panic = "abort"`) profiles, records their sizes and renders every page with every engine. |
| `reference.py` | Renders every page with headless Chrome (or Edge), offline, as the reference. |
| `report.py` | Side-by-side sheets per page (`sheets/`), a similarity score against Chrome and `summary.json`. |

## Running

```bash
python scripts/engine-compare/fetch_live.py target/engine-compare/live
bash scripts/engine-compare/run.sh          # in WSL on Windows
python scripts/engine-compare/reference.py
python scripts/engine-compare/report.py
```

Everything lands in `target/engine-compare/`. File names are fixed and free of
`:`, which Windows refuses.

## Reading the results

- The probes render with no network and no fetcher, so remote images do not
  load for any engine; Chrome is run with every host unresolvable for the same
  reason. Like the mail and help readers, the litehtml probe gets the page as a
  string, so it loads no linked style sheet or local image.
- The probes run on Linux; Chrome runs on Windows. Blitz draws with its bundled
  Liberation fonts, which share Arial's and Times New Roman's metrics, while
  litehtml and NetSurf use the Linux system fonts (DejaVu), so their text runs
  wider than Chrome's.
- The similarity score compares blurred quarter-size images pixel by pixel
  (within 24 levels). It catches layout, colour and missing content, not text
  rendering; treat a few points as noise.
- Sizes are whole stripped binaries; an engine's cost is its probe minus the
  baseline.
