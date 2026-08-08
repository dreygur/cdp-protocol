//! That the committed `methods/` modules still match the vendored schemas.
//!
//! This compares meaning rather than bytes: it reads the constants back out of
//! the committed source and checks the set against the schema. Byte comparison
//! would break the moment rustfmt wrapped a long constant, which it does.

use std::collections::BTreeSet;
use std::fs;

use xtask::names::{screaming_snake, snake};
use xtask::{repository_root, schema};

// -- The rules of the play -------------------------------------------------

/// Where the committed modules live, relative to the repository root.
const METHODS_DIR: &str = "crates/cdp-protocol/src/methods";

/// The shape every generated constant has, whatever rustfmt did to the line.
const CONST_PREFIX: &str = "pub const ";

// -- Leaves ----------------------------------------------------------------

/// Every `NAME = "Domain.method"` pair declared in the committed modules.
fn committed_constants() -> BTreeSet<(String, String)> {
    let dir = repository_root().join(METHODS_DIR);
    let mut found = BTreeSet::new();

    for entry in fs::read_dir(&dir).expect("read the methods directory") {
        let path = entry.expect("read a directory entry").path();
        if path.file_name().is_some_and(|name| name == "mod.rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read a module");

        // rustfmt may put the value on the next line, so join everything first
        // and then split on the declaration keyword.
        let flattened = source.split_whitespace().collect::<Vec<_>>().join(" ");
        for piece in flattened.split(CONST_PREFIX).skip(1) {
            let name = piece
                .split(':')
                .next()
                .expect("a constant always has a name")
                .to_string();
            let value = piece
                .split('"')
                .nth(1)
                .expect("a constant always has a string value")
                .to_string();
            found.insert((name, value));
        }
    }
    found
}

/// Every constant the schemas say should exist.
fn expected_constants() -> BTreeSet<(String, String)> {
    schema::load(&repository_root())
        .iter()
        .flat_map(|domain| {
            domain.commands.iter().map(move |command| {
                (
                    screaming_snake(&command.name),
                    format!("{}.{}", domain.domain, command.name),
                )
            })
        })
        .collect()
}

// -- Scene: the committed output is the schema's output --------------------

#[test]
fn every_schema_command_has_a_committed_constant() {
    let missing: Vec<_> = expected_constants()
        .difference(&committed_constants())
        .cloned()
        .collect();

    assert!(
        missing.is_empty(),
        "{} command(s) missing from the committed modules; run `just generate-methods`: {:?}",
        missing.len(),
        &missing[..missing.len().min(10)]
    );
}

#[test]
fn no_committed_constant_is_absent_from_the_schema() {
    let extra: Vec<_> = committed_constants()
        .difference(&expected_constants())
        .cloned()
        .collect();

    assert!(
        extra.is_empty(),
        "{} stale constant(s) in the committed modules; run `just generate-methods`: {:?}",
        extra.len(),
        &extra[..extra.len().min(10)]
    );
}

#[test]
fn every_domain_with_commands_has_a_module() {
    let dir = repository_root().join(METHODS_DIR);
    for domain in schema::load(&repository_root()) {
        if domain.commands.is_empty() {
            continue;
        }
        let module = dir.join(format!("{}.rs", snake(&domain.domain)));
        assert!(
            module.exists(),
            "no module for the {} domain at {}",
            domain.domain,
            module.display()
        );
    }
}

#[test]
fn the_index_publishes_every_module() {
    let dir = repository_root().join(METHODS_DIR);
    let index = fs::read_to_string(dir.join("mod.rs")).expect("read mod.rs");

    for entry in fs::read_dir(&dir).expect("read the methods directory") {
        let path = entry.expect("read a directory entry").path();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("a module file name");
        if name == "mod" {
            continue;
        }
        assert!(
            index.contains(&format!("pub mod {name};")),
            "mod.rs does not publish {name}"
        );
    }
}
