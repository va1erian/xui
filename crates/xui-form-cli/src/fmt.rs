#![forbid(unsafe_code)]

//! `xui-form fmt`: rewrite forms in the canonical layout.

use std::process::ExitCode;

use crate::args::{self, Args, Failure, Outcome};

/// Formats every file named in place, or with `--check` only lists the ones
/// that are not canonical (exiting 1 when there are any).
pub(crate) fn run(mut args: Args) -> Outcome {
    let check = args.flag("check");
    let files = args.positionals();
    args.finish()?;
    if files.is_empty() {
        return Err(Failure::Usage("`fmt` needs at least one file".to_owned()));
    }
    let mut unformatted = false;
    for file in files {
        let text = args::read(&file)?;
        let formatted = xui_form::format(&text)
            .map_err(|error| Failure::Fatal(format!("{}:{error}", file.display())))?;
        if formatted == text {
            continue;
        }
        unformatted = true;
        if check {
            println!("{}: not formatted", file.display());
        } else {
            args::write(&file, &formatted)?;
            println!("{}: formatted", file.display());
            if has_comments(&text) {
                eprintln!("{}: note: comments are not kept", file.display());
            }
        }
    }
    Ok(if check && unformatted {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// Whether `text` holds a `//` or `/* */` comment outside its strings:
/// formatting rewrites the form from its tree, which has no place for one.
fn has_comments(text: &str) -> bool {
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                // A string, to its closing quote.
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => {
                            chars.next();
                        }
                        '"' => break,
                        _ => {}
                    }
                }
            }
            'r' if matches!(chars.peek(), Some('#' | '"')) => {
                // A raw string: `r"..."`, `r#"..."#`.
                let mut hashes = 0;
                while chars.next_if_eq(&'#').is_some() {
                    hashes += 1;
                }
                if chars.next_if_eq(&'"').is_some() {
                    let mut closing = 0;
                    for c in chars.by_ref() {
                        match c {
                            '"' => closing = 1,
                            '#' if closing > 0 => closing += 1,
                            _ => closing = 0,
                        }
                        if closing > hashes {
                            break;
                        }
                    }
                }
            }
            '/' if matches!(chars.peek(), Some('/' | '*')) => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::has_comments;

    #[test]
    fn comments_are_found_outside_strings_only() {
        assert!(has_comments("Form() // a note"));
        assert!(has_comments("/* a note */ Form()"));
        assert!(!has_comments(
            r#"Form(title: "http://example.com", root: Column())"#
        ));
        assert!(!has_comments(r#"Form(title: "a \" // b")"#));
        assert!(!has_comments(r##"Form(title: r#"// not "a" comment"#)"##));
        assert!(has_comments(r##"Form(title: r#"x"#) // yes"##));
        assert!(!has_comments("Form(title: \"\")"));
    }
}
