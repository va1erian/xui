//! Every subcommand of the `xui-form` binary, run as a process on files in a
//! scratch directory. Rendering is headless, so nothing opens a window.

mod common;

use common::{HELLO, file, scratch, stderr, stdout, xui_form};

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
fn migrate_refuses_a_form_nested_too_deep_and_writes_nothing() {
    let dir = scratch("migrate-deep");
    let mut v1 = String::from("format = 1\n[window]\nname = \"deep\"\n");
    for level in 0..9 {
        v1.push_str(&format!(
            "[[node]]\nkind = \"Panel\"\nname = \"panel_{level}\"\nwidth = 180\nheight = 180\n"
        ));
        if level > 0 {
            v1.push_str(&format!("parent = \"panel_{}\"\n", level - 1));
        }
    }
    let source = file(&dir, "deep.lfm", &v1);
    let out = dir.join("deep-v2.lfm");
    let output = xui_form(&["migrate", &source, "--out", &out.to_string_lossy()]);
    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(message.contains("node `panel_7`"), "{message}");
    assert!(message.contains("too deeply"), "{message}");
    assert!(stdout(&output).is_empty(), "nothing is printed");
    assert!(!out.exists(), "nothing is written");
}

#[test]
fn migrate_reports_every_violation_by_node_and_writes_nothing() {
    let dir = scratch("migrate-invalid");
    let v1 = file(
        &dir,
        "bad.lfm",
        r#"format = 1
[window]
name = "bad"
[[node]]
kind = "Button"
name = "ok_button"
[[node]]
kind = "Label"
name = "inside_button"
parent = "ok_button"
[[node]]
kind = "Panel"
name = "loop_a"
parent = "loop_b"
[[node]]
kind = "Panel"
name = "loop_b"
parent = "loop_a"
[[node]]
kind = "Label"
name = "orphan"
parent = "nowhere"
[[node]]
kind = "Label"
name = "orphan"
"#,
    );
    let out = dir.join("bad-v2.lfm");
    let output = xui_form(&["migrate", &v1, "--out", &out.to_string_lossy()]);
    assert_eq!(output.status.code(), Some(1));
    let report = stderr(&output);
    for needle in [
        "error: node `inside_button`: its parent `ok_button` is a Button",
        "error: node `loop_a`: it is part of a parent cycle",
        "error: node `loop_b`: it is part of a parent cycle",
        "error: node `orphan`: its parent `nowhere` does not exist",
        "error: node `orphan`: the name is used more than once",
    ] {
        assert!(report.contains(needle), "{needle}\n{report}");
    }
    assert!(stdout(&output).is_empty());
    assert!(!out.exists(), "nothing is written");
}

#[test]
fn migrate_to_standard_output_prints_nothing_on_a_violation() {
    let dir = scratch("migrate-invalid-stdout");
    let v1 = file(
        &dir,
        "bad.lfm",
        "format = 1\n[window]\nname = \"bad\"\n[[node]]\nkind = \"Label\"\nname = \"a\"\nparent = \"a\"\n",
    );
    let output = xui_form(&["migrate", &v1]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
}

#[test]
fn a_huge_control_array_is_reported_and_never_built() {
    let dir = scratch("array-cap");
    let huge = file(
        &dir,
        "huge.lfm",
        r#"Form(root: Button(name: "b", array: 100000000))"#,
    );
    let output = xui_form(&["check", &huge]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stdout(&output).contains("a control array has at most 1024 elements"),
        "{}",
        stdout(&output)
    );

    // `render` validates first, with or without a script, and builds nothing.
    let script = file(&dir, "huge.rhai", "fn form_load() {}");
    let out = dir.join("out").to_string_lossy().into_owned();
    for extra in [vec![], vec!["--script", script.as_str()]] {
        let mut args = vec!["render", huge.as_str(), "--out", out.as_str()];
        args.extend(extra);
        let output = xui_form(&args);
        assert_eq!(output.status.code(), Some(1));
        let message = stderr(&output);
        assert!(
            message.contains("a control array has at most 1024 elements"),
            "{message}"
        );
        assert!(message.contains("not rendered"), "{message}");
    }
    assert!(!dir.join("out").exists(), "no image is written");
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

#[test]
fn migrate_refuses_a_form_that_would_not_pass_check_and_writes_nothing() {
    let dir = scratch("migrate-invalid-v2");
    let v1 = file(
        &dir,
        "old.lfm",
        r#"format = 1
[window]
name = "old"
[[node]]
kind = "Button"
name = "form"
[[node]]
kind = "Edit"
name = "my-edit"
[[node]]
kind = "Slider"
name = "level"
min = 50
max = 10
"#,
    );
    let out = dir.join("new.lfm");
    let output = xui_form(&["migrate", &v1, "--out", &out.to_string_lossy()]);
    assert_eq!(output.status.code(), Some(1));
    let report = stderr(&output);
    assert!(
        report.contains("node `form`: `form` is reserved"),
        "{report}"
    );
    assert!(
        report.contains("node `my-edit`: `my-edit` is not a valid name"),
        "{report}"
    );
    assert!(
        report.contains("node `level`: `min` (50) is greater than `max` (10)"),
        "{report}"
    );
    assert!(!out.exists(), "nothing is written");
}
