//! How to run this binary again, for the commands that hints print.
//!
//! A bare `paircaster` in a hint fails when the binary is not on `PATH`, which
//! is the usual case while trying it from a checkout. So a hint names the
//! binary the way it was run: `paircaster` if it was found on `PATH`, else its
//! absolute path, which still works from another directory. A `node` hint
//! also names `--dir` when the install is not the default one.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::snapshot;

/// The command that runs this binary, ready to paste into a shell.
pub fn paircaster() -> String {
    let argv0 = std::env::args_os().next();
    command(argv0.as_deref(), std::env::current_exe().ok().as_deref())
}

/// `paircaster node <sub>` for the install at `dir`.
pub fn node(sub: &str, dir: &Path) -> String {
    let default = snapshot::install_dir(None).ok();
    node_command(&paircaster(), sub, dir, default.as_deref())
}

/// `path`, ready to paste into a shell.
pub fn path(path: &Path) -> String {
    quote(&path.to_string_lossy())
}

fn node_command(paircaster: &str, sub: &str, dir: &Path, default: Option<&Path>) -> String {
    let dir = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    match default == Some(dir.as_path()) {
        true => format!("{paircaster} node {sub}"),
        false => format!("{paircaster} node {sub} --dir {}", path(&dir)),
    }
}

fn command(argv0: Option<&OsStr>, exe: Option<&Path>) -> String {
    let typed = argv0.map(Path::new);
    let on_path = typed.is_some_and(|p| p.components().count() == 1);
    let chosen: Option<PathBuf> = match on_path {
        true => typed.map(Path::to_path_buf),
        false => exe.or(typed).map(Path::to_path_buf),
    };
    chosen.map_or_else(|| "paircaster".to_owned(), |p| path(&p))
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

    const EXE: &str = "/Users/a/paircaster/target/release/paircaster";

    fn run(argv0: &str, exe: Option<&str>) -> String {
        command(Some(OsStr::new(argv0)), exe.map(Path::new))
    }

    #[test]
    fn found_on_path_stays_bare() {
        assert_eq!(
            run("paircaster", Some("/usr/local/bin/paircaster")),
            "paircaster"
        );
    }

    #[test]
    fn run_by_relative_path_becomes_absolute() {
        assert_eq!(run("./target/release/paircaster", Some(EXE)), EXE);
    }

    #[test]
    fn run_by_path_without_current_exe_keeps_what_was_typed() {
        assert_eq!(
            run("./target/release/paircaster", None),
            "./target/release/paircaster"
        );
    }

    #[test]
    fn no_argv0_falls_back_to_the_name() {
        assert_eq!(command(None, Some(Path::new(EXE))), EXE);
        assert_eq!(command(None, None), "paircaster");
    }

    #[test]
    fn the_default_install_needs_no_dir() {
        let home = Path::new("/Users/a/snapchain");
        assert_eq!(
            node_command(
                "paircaster",
                "resume",
                Path::new("/Users/a/snapchain/"),
                Some(home)
            ),
            "paircaster node resume"
        );
    }

    #[test]
    fn another_install_is_named_and_quoted() {
        let home = Path::new("/Users/a/snapchain");
        assert_eq!(
            node_command(
                "paircaster",
                "pause",
                Path::new("/Volumes/Big Disk/sc"),
                Some(home)
            ),
            "paircaster node pause --dir '/Volumes/Big Disk/sc'"
        );
        assert_eq!(
            node_command("paircaster", "status", Path::new("/data/sc"), None),
            "paircaster node status --dir /data/sc"
        );
    }

    #[test]
    fn paths_with_spaces_and_quotes_are_quoted() {
        assert_eq!(
            run("./x", Some("/Users/a b/it's/paircaster")),
            r"'/Users/a b/it'\''s/paircaster'"
        );
    }
}
