//! With the `viewer` feature, the binary embeds the viewer's build. Refuse
//! early, and say how to make it, when the build is not there.

const FILES: [&str; 2] = ["index.html", "viewer.js"];

fn main() {
    if std::env::var_os("CARGO_FEATURE_VIEWER").is_none() {
        return;
    }
    let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../viewer/dist");
    for file in FILES {
        let path = dist.join(file);
        println!("cargo::rerun-if-changed={}", path.display());
        assert!(
            path.is_file(),
            "the viewer feature needs {}: run `bun install --frozen-lockfile && bun build.ts` in viewer/ first",
            path.display()
        );
    }
}
