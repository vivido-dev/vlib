//! The same name means the same example in all three languages.
//!
//! This project ships one gallery three times: a Rust crate, a Python package, and a TypeScript
//! one. Their value is that a reader can hold two of them side by side, so an example added to
//! one and forgotten in the others is a defect in the set rather than a gap in a language.
//!
//! Nothing else notices that. Each language's own tests check what its examples do, and all of
//! them pass perfectly well when a name exists in only one place — which is exactly how the
//! three drift apart.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// The one example with no port, and why.
///
/// `svg` rasterizes a single vector document at several device sizes, which is the whole lesson —
/// a scaled stroke that stays sharp. The Rust crate has resvg; neither the Python nor the
/// TypeScript half has a rasterizer, and neither standard library offers one. Drawing the two
/// documents as hand-written paths would port the picture and lose the point.
const RUST_ONLY: [&str; 1] = ["svg"];

fn names(directory: &str, extension: &str) -> BTreeSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(directory);
    let entries = fs::read_dir(&path).unwrap_or_else(|error| {
        panic!("cannot read {}: {error}", path.display());
    });
    entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            let stem = name.strip_suffix(extension)?;
            // A helper or a config file is not an example.
            (!stem.starts_with('_') && !stem.starts_with('.')).then(|| stem.to_owned())
        })
        .collect()
}

#[test]
fn every_example_exists_in_all_three_languages() {
    let rust = names("examples", ".rs");
    let python = names("examples/python", ".py");
    let typescript = names("examples/typescript", ".ts");

    assert!(!rust.is_empty(), "no Rust examples were found at all");

    let ported: BTreeSet<String> = rust
        .iter()
        .filter(|name| !RUST_ONLY.contains(&name.as_str()))
        .cloned()
        .collect();

    let missing_from_python: Vec<&String> = ported.difference(&python).collect();
    assert!(
        missing_from_python.is_empty(),
        "these Rust examples have no Python twin: {missing_from_python:?}"
    );

    let missing_from_typescript: Vec<&String> = ported.difference(&typescript).collect();
    assert!(
        missing_from_typescript.is_empty(),
        "these Rust examples have no TypeScript twin: {missing_from_typescript:?}"
    );

    // And nothing exists in a port that the gallery does not have, which would be an example
    // nobody can compare against anything.
    let stray_python: Vec<&String> = python.difference(&rust).collect();
    assert!(
        stray_python.is_empty(),
        "these Python examples have no Rust original: {stray_python:?}"
    );
    let stray_typescript: Vec<&String> = typescript.difference(&rust).collect();
    assert!(
        stray_typescript.is_empty(),
        "these TypeScript examples have no Rust original: {stray_typescript:?}"
    );

    // The exception has to stay an exception rather than quietly becoming a habit.
    for only in RUST_ONLY {
        assert!(
            rust.contains(only),
            "{only} is listed as Rust-only but no longer exists"
        );
    }
}

#[test]
fn every_example_has_the_view_it_runs() {
    // A binary and its view are a pair: the binary is the window, the view is the UI, and the
    // tests include the view by path so they drive the code the example runs rather than a copy.
    let rust = names("examples", ".rs");
    let views = names("examples/views", ".rs");
    let orphans: Vec<&String> = rust.difference(&views).collect();
    assert!(
        orphans.is_empty(),
        "these examples have no view: {orphans:?}"
    );
}
