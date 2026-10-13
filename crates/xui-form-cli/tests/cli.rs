//! Every subcommand of the `xui-form` binary, run as a process on files in a
//! scratch directory. Rendering is headless, so nothing opens a window.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fresh scratch directory for `test`.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("xui-form-cli")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory is created");
    dir
}

/// Runs `xui-form` with `args`.
fn xui_form(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xui-form"))
        .args(args)
        .output()
        .expect("xui-form runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Writes `text` to `dir/name` and returns the path as a string.
fn file(dir: &Path, name: &str, text: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("the file is written");
    path.to_string_lossy().into_owned()
}

const HELLO: &str = r#"Form(title: "Hello", size: (240, 120), root: Column(gap: 8, padding: 16, children: [
    Edit(name: "name_edit", placeholder: "Your name"),
    Button(name: "greet_button", text: "Greet"),
    Label(name: "result_label"),
]))
"#;

#[test]
fn check_passes_a_valid_form_and_fails_a_broken_one() {
    let dir = scratch("check");
    let good = file(&dir, "good.lfm", HELLO);
    let output = xui_form(&["check", &good]);
    assert!(output.status.success());
    assert!(
        stdout(&output).ends_with("good.lfm: ok\n"),
        "{}",
        stdout(&output)
    );

    let misspelt = file(&dir, "misspelt.lfm", "Form(root: Button(txt: \"OK\"))");
    let output = xui_form(&["check", &misspelt]);
    assert_eq!(output.status.code(), Some(1));
    let report = stdout(&output);
    assert!(report.contains("misspelt.lfm:1:19: error:"), "{report}");
    assert!(report.contains("(did you mean `text`?)"), "{report}");

    let invalid = file(
        &dir,
        "invalid.lfm",
        r#"Form(root: Row(children: [Label(name: "a"), Label(name: "a")]))"#,
    );
    let output = xui_form(&["check", &invalid]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("error: `a`: `a` is used more than once"));
}

#[test]
fn render_writes_light_and_dark_pngs_and_a_layout_report() {
    let dir = scratch("render");
    let form = file(&dir, "hello.lfm", HELLO);
    let script = file(
        &dir,
        "hello.rhai",
        "fn form_load() { result_label.text = \"Hi\"; }",
    );
    let out = dir.join("out");
    let output = xui_form(&[
        "render",
        &form,
        "--out",
        &out.to_string_lossy(),
        "--script",
        &script,
        "--report",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = stdout(&output);
    assert!(report.contains("Label \"Hi\""), "the script ran: {report}");
    for variant in ["light", "dark"] {
        let png = std::fs::read(out.join(format!("hello-{variant}.png"))).expect("the PNG exists");
        assert_eq!(&png[1..4], b"PNG");
    }
}

#[test]
fn fmt_rewrites_a_form_and_check_reports_it() {
    let dir = scratch("fmt");
    let messy = file(
        &dir,
        "messy.lfm",
        "Form(size:(240,120),title:\"Hello\",root:Column(padding:16,gap:8,children:[Edit(name:\"name_edit\",placeholder:\"Your name\"),Button(name:\"greet_button\",text:\"Greet\"),Label(name:\"result_label\",text:\"\")]))",
    );
    let output = xui_form(&["fmt", "--check", &messy]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("not formatted"));

    assert!(xui_form(&["fmt", &messy]).status.success());
    assert_eq!(std::fs::read_to_string(&messy).expect("it reads"), HELLO);
    assert!(xui_form(&["fmt", "--check", &messy]).status.success());
}

#[test]
fn migrate_converts_a_format_one_form() {
    let dir = scratch("migrate");
    let v1 = file(
        &dir,
        "old.lfm",
        "format = 1\n\n[window]\nname = \"main_form\"\ntitle = \"Old\"\n\n[[node]]\nkind = \"Button\"\nname = \"ok\"\nleft = 200\ntop = 150\nwidth = 80\nheight = 28\nanchor = \"bottom_right\"\ntext = \"OK\"\ntab_index = 2\n",
    );
    let output = xui_form(&["migrate", &v1]);
    assert!(output.status.success());
    let converted = stdout(&output);
    assert!(
        converted.starts_with("Form(name: \"main_form\", title: \"Old\""),
        "{converted}"
    );
    assert!(
        converted.contains("at: (200, 150, 80, 28), anchor: BottomRight"),
        "{converted}"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("tab_index"));

    let out = dir.join("new.lfm");
    assert!(
        xui_form(&["migrate", &v1, "--out", &out.to_string_lossy()])
            .status
            .success()
    );
    let check = xui_form(&["check", &out.to_string_lossy()]);
    assert!(check.status.success(), "{}", stdout(&check));
}

#[test]
fn schema_prints_every_kind_as_json() {
    let output = xui_form(&["schema", "--json"]);
    assert!(output.status.success());
    let schema: serde_json::Value = serde_json::from_slice(&output.stdout).expect("it is JSON");
    let button = &schema["widgets"]["Button"];
    assert_eq!(button["events"][0]["name"], "Click");
    assert_eq!(button["properties"][0]["name"], "text");
    assert!(schema["layouts"].as_array().is_some_and(|l| l.len() == 5));
    assert_eq!(schema["layout_fields"][0]["name"], "fill");
}

#[test]
fn a_misspelt_command_suggests_the_closest() {
    let output = xui_form(&["chek", "form.lfm"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("did you mean `check`?"));
}
