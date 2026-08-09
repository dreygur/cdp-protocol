//! That the committed `methods/` and `protocol/` modules still match the
//! vendored schemas.
//!
//! This compares meaning rather than bytes: it reads the declarations back out
//! of the committed source and checks them against the schema. Byte comparison
//! would break the moment rustfmt wrapped a long constant, which it does.

use std::collections::BTreeSet;
use std::fs;

use xtask::names::{pascal, screaming_snake, snake};
use xtask::{repository_root, schema};

/// Where the committed modules live, relative to the repository root.
const METHODS_DIR: &str = "crates/cdp-protocol/src/methods";

/// Where the committed typed modules live, relative to the repository root.
const PROTOCOL_DIR: &str = "crates/cdp-protocol/src/protocol";

/// The shape every generated constant has, whatever rustfmt did to the line.
const CONST_PREFIX: &str = "pub const ";

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

/// Every committed typed module, whitespace flattened, so a declaration can be
/// looked for whatever rustfmt did to the lines it spans.
fn committed_protocol() -> String {
    let dir = repository_root().join(PROTOCOL_DIR);
    let mut out = String::new();

    for entry in fs::read_dir(&dir).expect("read the protocol directory") {
        let path = entry.expect("read a directory entry").path();
        let source = fs::read_to_string(&path).expect("read a module");
        out.push_str(&source.split_whitespace().collect::<Vec<_>>().join(" "));
        out.push(' ');
    }
    out
}

#[test]
fn every_schema_command_is_tied_to_its_own_method_and_result() {
    let committed = committed_protocol();
    let mut missing: Vec<String> = Vec::new();

    for domain in schema::load(&repository_root()) {
        for command in &domain.commands {
            let base = pascal(&command.name);
            let returns = if command.returns.is_empty() {
                "crate::typed::NoReturns".to_string()
            } else {
                format!("{base}Returns")
            };
            let expected = format!(
                "impl crate::typed::Command for {base}Params {{ \
                 const METHOD: &'static str = \"{}.{}\"; type Returns = {returns}; }}",
                domain.domain, command.name
            );
            if !committed.contains(&expected) {
                missing.push(expected);
            }
        }
    }

    assert!(
        missing.is_empty(),
        "{} command(s) are not tied to their result; run `just generate-methods`: {:?}",
        missing.len(),
        &missing[..missing.len().min(5)]
    );
}

#[test]
fn every_schema_event_has_a_payload_that_knows_its_name() {
    let committed = committed_protocol();
    let mut missing: Vec<String> = Vec::new();

    for domain in schema::load(&repository_root()) {
        for event in &domain.events {
            let expected = format!(
                "impl crate::typed::Event for {}Event {{ \
                 const METHOD: &'static str = \"{}.{}\"; }}",
                pascal(&event.name),
                domain.domain,
                event.name
            );
            if !committed.contains(&expected) {
                missing.push(expected);
            }
        }
    }

    assert!(
        missing.is_empty(),
        "{} event(s) have no committed payload; run `just generate-methods`: {:?}",
        missing.len(),
        &missing[..missing.len().min(5)]
    );
}

#[test]
fn every_schema_type_has_a_committed_declaration() {
    let committed = committed_protocol();
    let mut missing: Vec<String> = Vec::new();

    for domain in schema::load(&repository_root()) {
        for declared in &domain.types {
            let name = pascal(&declared.id);
            let spellings = [
                format!("pub struct {name} {{"),
                format!("pub enum {name} {{"),
                format!("pub type {name} ="),
            ];
            if !spellings.iter().any(|shape| committed.contains(shape)) {
                missing.push(format!("{}.{}", domain.domain, declared.id));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "{} type(s) are not declared; run `just generate-methods`: {:?}",
        missing.len(),
        &missing[..missing.len().min(5)]
    );
}

#[test]
fn nothing_typed_is_committed_that_the_schema_does_not_declare() {
    let committed = committed_protocol();
    let domains = schema::load(&repository_root());

    let commands: usize = domains.iter().map(|domain| domain.commands.len()).sum();
    let events: usize = domains.iter().map(|domain| domain.events.len()).sum();

    assert_eq!(
        committed.matches("impl crate::typed::Command for").count(),
        commands,
        "the committed modules and the schema disagree about how many commands exist"
    );
    assert_eq!(
        committed.matches("impl crate::typed::Event for").count(),
        events,
        "the committed modules and the schema disagree about how many events exist"
    );
}

#[test]
fn every_domain_has_a_typed_module_the_index_publishes() {
    let dir = repository_root().join(PROTOCOL_DIR);
    let index = fs::read_to_string(dir.join("mod.rs")).expect("read mod.rs");

    for domain in schema::load(&repository_root()) {
        let module = snake(&domain.domain);
        assert!(
            dir.join(format!("{module}.rs")).exists(),
            "no typed module for the {} domain",
            domain.domain
        );
        assert!(
            index.contains(&format!("pub mod {module};")),
            "mod.rs does not publish {module}"
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
