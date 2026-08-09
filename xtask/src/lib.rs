//! Repository automation: regenerating `crates/cdp-protocol/src/methods/` and
//! `crates/cdp-protocol/src/protocol/` from the vendored CDP schemas.
//!
//! Every CDP command becomes a `&str` constant carrying the browser's own
//! description, its parameters and its return fields as rustdoc. That turns the
//! method name passed to `CdpClient::call` from a string the compiler cannot
//! check into a name the editor can complete and the compiler can reject.
//!
//! Alongside it, every command also becomes a parameters struct tied to its
//! result type, every event a payload struct, and every named type in the
//! protocol a Rust type, so that a caller never has to hand-write the JSON.
//!
//! The output is committed, so building the crate never runs this tool and
//! docs.rs shows the generated documentation.

pub mod docs;
pub mod names;
pub mod render;
pub mod render_protocol;
pub mod schema;
pub mod types;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::schema::Domain;

/// Where the generated method-name modules are written, relative to the root.
const METHODS_DIR: &str = "crates/cdp-protocol/src/methods";

/// Where the generated typed-protocol modules are written, relative to the root.
const PROTOCOL_DIR: &str = "crates/cdp-protocol/src/protocol";

/// What one generation run produced, for reporting and for tests.
#[derive(Debug, PartialEq, Eq)]
pub struct Generated {
    pub commands: usize,
    pub domains: usize,
    pub types: usize,
    pub events: usize,
}

/// The repository root, found by walking up from this crate.
pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask always sits one level below the repository root")
        .to_path_buf()
}

/// Empty a generated directory, so a domain the schema dropped does not linger.
fn fresh(directory: &Path) {
    if directory.exists() {
        fs::remove_dir_all(directory).expect("clear the previous generation");
    }
    fs::create_dir_all(directory).expect("create a generated directory");
}

/// Write the method-name modules, one per domain that has commands.
///
/// Domains with no commands are skipped: a module of nothing helps nobody.
fn generate_methods(root: &Path, domains: &[Domain]) -> Generated {
    let output = root.join(METHODS_DIR);
    fresh(&output);

    let mut modules: BTreeMap<String, String> = BTreeMap::new();
    let mut commands = 0;

    for domain in domains {
        if domain.commands.is_empty() {
            continue;
        }
        let module = names::snake(&domain.domain);
        fs::write(output.join(format!("{module}.rs")), render::domain(domain))
            .expect("write a domain module");
        commands += domain.commands.len();
        modules.insert(module, domain.domain.clone());
    }

    fs::write(output.join("mod.rs"), render::index(&modules)).expect("write the index module");

    Generated {
        commands,
        domains: modules.len(),
        types: 0,
        events: 0,
    }
}

/// Write the typed modules, one per domain that declares anything at all.
fn generate_protocol(root: &Path, domains: &[Domain]) -> Generated {
    let output = root.join(PROTOCOL_DIR);
    fresh(&output);

    let registry = types::registry(domains);
    let mut modules: BTreeMap<String, String> = BTreeMap::new();
    let mut counted = Generated {
        commands: 0,
        domains: 0,
        types: 0,
        events: 0,
    };

    for domain in domains {
        if domain.commands.is_empty() && domain.types.is_empty() && domain.events.is_empty() {
            continue;
        }
        let module = names::snake(&domain.domain);
        fs::write(
            output.join(format!("{module}.rs")),
            render_protocol::domain(&registry, domain),
        )
        .expect("write a typed domain module");
        counted.commands += domain.commands.len();
        counted.types += domain.types.len();
        counted.events += domain.events.len();
        modules.insert(module, domain.domain.clone());
    }

    fs::write(output.join("mod.rs"), render_protocol::index(&modules))
        .expect("write the typed index module");

    counted.domains = modules.len();
    counted
}

/// Regenerate every committed module under `root`, replacing what was there.
pub fn generate(root: &Path) -> Generated {
    let domains = schema::load(root);
    let methods = generate_methods(root, &domains);
    let protocol = generate_protocol(root, &domains);

    assert_eq!(
        methods.commands, protocol.commands,
        "the two generators disagree about how many commands the schema declares"
    );
    protocol
}
