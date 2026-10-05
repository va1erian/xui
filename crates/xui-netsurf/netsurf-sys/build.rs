//! Builds the NetSurf core and the eight NetSurf libraries it needs from the
//! sources vendored in `vendor/`, plus the nsx glue in `csrc/`, into one
//! static library.
//!
//! The file lists in `sources/` are what NetSurf's own Makefiles compile for a
//! frontend-less build with curl, JavaScript, PNG/JPEG/WebP, SVG, utf8proc,
//! libnslog and libnspsl turned off (the glue brings its own http(s) fetcher
//! and PNG/JPEG handlers, which hand the work to the host) (`scripts/regen.sh` records how they were
//! made). Files the libraries generate at build time with perl, gperf and
//! small host tools are committed under `generated/`, so this script needs only
//! a C compiler.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// The defines every NetSurf library build passes (`buildsystem`'s defaults).
const LIB_DEFINES: &[(&str, &str)] = &[
    ("_BSD_SOURCE", "1"),
    ("_DEFAULT_SOURCE", "1"),
    ("STMTEXPR", "1"),
    ("_ALIGNED", "__attribute__((aligned))"),
    ("NDEBUG", "1"),
];

/// The libraries in link order: each only uses those after it.
const LIBS: &[&str] = &[
    "libdom",
    "libcss",
    "libhubbub",
    "libparserutils",
    "libnsgif",
    "libnsbmp",
    "libnsutils",
    "libwapcaplet",
];

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let vendor = root.join("vendor");
    let generated = root.join("generated");
    if !vendor.join("netsurf/desktop/netsurf.c").exists() {
        panic!("netsurf-sys: vendor/ is incomplete; run `scripts/vendor.sh`");
    }

    let zlib_include = env::var_os("DEP_Z_INCLUDE").map(PathBuf::from);
    let mut lib_includes: Vec<PathBuf> = LIBS
        .iter()
        .map(|lib| vendor.join(lib).join("include"))
        .collect();
    lib_includes.push(install_dom_bindings(&vendor));

    // Each `compile` emits its link line, and static archives resolve left
    // to right: the core first, then the libraries in dependency order.
    build_netsurf(&root, &vendor, &generated, &lib_includes, zlib_include);
    for lib in LIBS {
        build_library(&root, &vendor, &generated, lib, &lib_includes);
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=csrc");
    println!("cargo:rerun-if-changed=sources");
    println!("cargo:rerun-if-changed=generated");
    // The vendored sources: a moved pin must rebuild the archives.
    println!("cargo:rerun-if-changed=vendor");
}

/// Copies libdom's HTML-parser binding headers to where its `make install`
/// puts them (`<dom/bindings/hubbub/parser.h>`), returning the include root.
fn install_dom_bindings(vendor: &Path) -> PathBuf {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("include");
    let dest = out.join("dom/bindings/hubbub");
    fs::create_dir_all(&dest).expect("netsurf-sys: creating the bindings include dir");
    for header in ["parser.h", "errors.h"] {
        fs::copy(
            vendor.join("libdom/bindings/hubbub").join(header),
            dest.join(header),
        )
        .expect("netsurf-sys: copying a libdom binding header");
    }
    out
}

/// Reads a `sources/<name>.txt` list: one path per line, relative to the
/// library's checkout.
fn source_list(root: &Path, name: &str) -> Vec<String> {
    let path = root.join("sources").join(format!("{name}.txt"));
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("netsurf-sys: reading {}: {e}", path.display()))
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// A listed file from the checkout, or from `generated/` when the library
/// writes it at build time.
fn locate(vendor: &Path, generated: &Path, lib: &str, file: &str) -> PathBuf {
    let checked_in = vendor.join(lib).join(file);
    if checked_in.exists() {
        return checked_in;
    }
    let made = generated.join(lib).join(file);
    if made.exists() {
        return made;
    }
    panic!("netsurf-sys: {lib}/{file} is neither in the checkout nor in generated/");
}

fn base_build() -> cc::Build {
    let mut build = cc::Build::new();
    build
        .std("c99")
        .warnings(false)
        .extra_warnings(false)
        .opt_level(2)
        .flag_if_supported("-fno-strict-aliasing");
    build
}

fn build_library(root: &Path, vendor: &Path, generated: &Path, lib: &str, includes: &[PathBuf]) {
    let mut build = base_build();
    for (key, value) in LIB_DEFINES {
        build.define(key, *value);
    }
    match lib {
        "libnsutils" => {
            build.define("_GNU_SOURCE", None);
            build.define("_POSIX_C_SOURCE", "200809L");
        }
        "libnsgif" => {
            build.define("NSGIF_NAME", "nsgif");
            build.define("NSGIF_VERSION", "1.0.0");
        }
        _ => {}
    }
    let lib_dir = vendor.join(lib);
    let gen_dir = generated.join(lib);
    build.include(lib_dir.join("src"));
    build.include(gen_dir.join("src"));
    // `#include "aliases.inc"` and `"entities.inc"` resolve beside the
    // including file in NetSurf's tree; here they are generated elsewhere.
    build.include(gen_dir.join("src/charset"));
    build.include(gen_dir.join("src/tokeniser"));
    for include in includes {
        build.include(include);
    }
    for file in source_list(root, lib) {
        build.file(locate(vendor, generated, lib, &file));
    }
    build.compile(lib);
}

fn build_netsurf(
    root: &Path,
    vendor: &Path,
    generated: &Path,
    includes: &[PathBuf],
    zlib_include: Option<PathBuf>,
) {
    let ns = vendor.join("netsurf");
    let mut build = base_build();
    for (key, value) in [
        ("_POSIX_C_SOURCE", "200809L"),
        ("_XOPEN_SOURCE", "700"),
        ("_BSD_SOURCE", "1"),
        ("_DEFAULT_SOURCE", "1"),
        ("_NETBSD_SOURCE", "1"),
        ("STMTEXPR", "1"),
        ("NDEBUG", "1"),
        ("WITH_BMP", "1"),
        ("WITH_GIF", "1"),
        ("NETSURF_LOG_LEVEL", "WARNING"),
        (
            "NETSURF_UA_FORMAT_STRING",
            "\"Mozilla/5.0 (%s) NetSurf/%d.%d\"",
        ),
        ("NETSURF_HOMEPAGE", "\"about:blank\""),
        ("NETSURF_BUILTIN_LOG_FILTER", "\"level:WARNING\""),
        ("NETSURF_BUILTIN_VERBOSE_FILTER", "\"level:VERBOSE\""),
    ] {
        build.define(key, value);
    }
    // The shim directory comes first so its stand-in headers win.
    build.include(generated.join("shim"));
    build.include(&ns);
    build.include(ns.join("include"));
    build.include(ns.join("frontends"));
    build.include(ns.join("content/handlers"));
    build.include(generated.join("netsurf"));
    build.include(root.join("csrc"));
    for include in includes {
        build.include(include);
    }
    if let Some(dir) = zlib_include {
        build.include(dir);
    }
    for file in source_list(root, "netsurf") {
        build.file(locate(vendor, generated, "netsurf", &file));
    }
    for glue in [
        "nsx_core.c",
        "nsx_window.c",
        "nsx_plot.c",
        "nsx_fetch.c",
        "nsx_post.c",
        "nsx_image.c",
    ] {
        build.file(root.join("csrc").join(glue));
    }
    build.compile("netsurf");
}
