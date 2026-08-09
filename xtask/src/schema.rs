//! The shape of the vendored CDP schemas, and how to read them.

use std::fs;
use std::path::Path;

use serde::Deserialize;

/// The schemas this generator reads, relative to the repository root.
const SCHEMAS: [&str; 2] = [
    "protocol/browser_protocol.json",
    "protocol/js_protocol.json",
];

/// The element type of an array, which the schema states the same two ways a
/// field does but without a name of its own.
#[derive(Debug, Deserialize)]
pub struct Items {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    #[serde(rename = "$ref")]
    pub reference: Option<String>,
}

/// A parameter or a return value. CDP names its type either inline as `type`
/// or as a cross-reference in `$ref`.
#[derive(Debug, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    #[serde(rename = "$ref")]
    pub reference: Option<String>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub experimental: bool,
    pub items: Option<Items>,
    /// The values this field accepts, when the schema states them here rather
    /// than in a named type of its own.
    #[serde(rename = "enum")]
    pub values: Option<Vec<String>>,
}

impl Field {
    /// How this field's type is written in the schema.
    pub fn type_name(&self) -> String {
        self.reference
            .clone()
            .or_else(|| self.kind.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// One named type within a domain: an object, a string enum, or an alias for a
/// primitive or an array.
#[derive(Debug, Deserialize)]
pub struct Type {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub experimental: bool,
    #[serde(default)]
    pub properties: Vec<Field>,
    pub items: Option<Items>,
    #[serde(rename = "enum")]
    pub values: Option<Vec<String>>,
}

/// One command within a domain.
#[derive(Debug, Deserialize)]
pub struct Command {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub experimental: bool,
    #[serde(default)]
    pub parameters: Vec<Field>,
    #[serde(default)]
    pub returns: Vec<Field>,
}

/// One event a domain can raise.
#[derive(Debug, Deserialize)]
pub struct Event {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub experimental: bool,
    #[serde(default)]
    pub parameters: Vec<Field>,
}

/// One CDP domain as the schema describes it.
#[derive(Debug, Deserialize)]
pub struct Domain {
    pub domain: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub experimental: bool,
    #[serde(default)]
    pub commands: Vec<Command>,
    #[serde(default)]
    pub types: Vec<Type>,
    #[serde(default)]
    pub events: Vec<Event>,
}

/// The top level of a schema file.
#[derive(Debug, Deserialize)]
struct Schema {
    domains: Vec<Domain>,
}

/// Read every vendored schema, returning its domains sorted by name so that
/// regenerating produces a stable diff.
pub fn load(root: &Path) -> Vec<Domain> {
    let mut domains = Vec::new();
    for relative in SCHEMAS {
        let path = root.join(relative);
        let text =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let schema: Schema =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
        domains.extend(schema.domains);
    }
    domains.sort_by(|a, b| a.domain.cmp(&b.domain));
    domains
}
