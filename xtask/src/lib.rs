//! Repository automation: regenerating `crates/cdp-protocol/src/methods/` from
//! the vendored CDP schemas.
//!
//! Every CDP command becomes a `&str` constant carrying the browser's own
//! description, its parameters and its return fields as rustdoc. That turns the
//! method name passed to `CdpClient::call` from a string the compiler cannot
//! check into a name the editor can complete and the compiler can reject.
//!
//! The output is committed, so building the crate never runs this tool and
//! docs.rs shows the generated documentation.

pub mod docs;
pub mod names;
pub mod render;
pub mod schema;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Where the generated modules are written, relative to the repository root.
const OUTPUT_DIR: &str = "crates/cdp-protocol/src/methods";

/// What one generation run produced, for reporting and for tests.
#[derive(Debug, PartialEq, Eq)]
pub struct Generated {
    pub commands: usize,
    pub domains: usize,
}

/// The repository root, found by walking up from this crate.
pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask always sits one level below the repository root")
        .to_path_buf()
}

/// Regenerate the method modules under `root`, replacing whatever was there.
///
/// Domains with no commands are skipped: a module of nothing helps nobody.
pub fn generate(root: &Path) -> Generated {
    let output = root.join(OUTPUT_DIR);
    if output.exists() {
        fs::remove_dir_all(&output).expect("clear the previous generation");
    }
    fs::create_dir_all(&output).expect("create the methods directory");

    let domains = schema::load(root);
    let mut modules: BTreeMap<String, String> = BTreeMap::new();
    let mut commands = 0;

    for domain in &domains {
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
    }
}
