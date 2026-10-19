//! How to run this binary again, for the commands that hints print.
//!
//! A bare `selfcast` in a hint fails when the binary is not on `PATH`, which
//! is the usual case while trying it from a checkout. So a hint names the
//! binary the way it was run: `selfcast` if it was found on `PATH`, else its
//! absolute path, which still works from another directory.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The command that runs this binary, ready to paste into a shell.
pub fn selfcast() -> String {
    let argv0 = std::env::args_os().next();
    command(argv0.as_deref(), std::env::current_exe().ok().as_deref())
}

fn command(argv0: Option<&OsStr>, exe: Option<&Path>) -> String {
    let typed = argv0.map(Path::new);
    let on_path = typed.is_some_and(|p| p.components().count() == 1);
    let chosen: Option<PathBuf> = match on_path {
        true => typed.map(Path::to_path_buf),
        false => exe.or(typed).map(Path::to_path_buf),
    };
    chosen.map_or_else(|| "selfcast".to_owned(), |p| quote(&p.to_string_lossy()))
}

/// `s` single-quoted for a POSIX shell, unless every character is safe bare.
fn quote(s: &str) -> String {
    let safe = |c: char| c.is_ascii_alphanumeric() || "/._-+~:@%".contains(c);
    match !s.is_empty() && s.chars().all(safe) {
        true => s.to_owned(),
        false => format!("'{}'", s.replace('\'', r"'\''")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = "/Users/a/selfcast/target/release/selfcast";

    fn run(argv0: &str, exe: Option<&str>) -> String {
        command(Some(OsStr::new(argv0)), exe.map(Path::new))
    }

    #[test]
    fn found_on_path_stays_bare() {
        assert_eq!(run("selfcast", Some("/usr/local/bin/selfcast")), "selfcast");
    }

    #[test]
    fn run_by_relative_path_becomes_absolute() {
        assert_eq!(run("./target/release/selfcast", Some(EXE)), EXE);
    }

    #[test]
    fn run_by_path_without_current_exe_keeps_what_was_typed() {
        assert_eq!(
            run("./target/release/selfcast", None),
            "./target/release/selfcast"
        );
    }

    #[test]
    fn no_argv0_falls_back_to_the_name() {
        assert_eq!(command(None, Some(Path::new(EXE))), EXE);
        assert_eq!(command(None, None), "selfcast");
    }

    #[test]
    fn paths_with_spaces_and_quotes_are_quoted() {
        assert_eq!(
            run("./x", Some("/Users/a b/it's/selfcast")),
            r"'/Users/a b/it'\''s/selfcast'"
        );
    }
}
