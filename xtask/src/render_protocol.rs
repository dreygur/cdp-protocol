//! Rendering a domain's types, command parameters and events as Rust source.

use std::collections::BTreeMap;

use crate::docs::{defuse, wrap};
use crate::names::pascal;
use crate::render::BANNER;
use crate::schema::{Command, Domain, Event, Field, Items, Type};
use crate::types::{field_name, inline_enum_name, variant_name, Owner, Registry};

/// What every generated struct derives. `PartialEq` is here because a decoded
/// payload is worth comparing in a test; `Eq` is not, because CDP has `number`
/// fields and those are `f64`.
const STRUCT_DERIVE: &str = "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]";

/// What a struct whose every field is optional derives instead, so a caller can
/// write `..Default::default()` and a command with no parameters at all can be
/// sent without naming a single field.
const DEFAULTABLE_DERIVE: &str =
    "#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]";

/// What every generated enum derives. An enum is only a string on the wire, so
/// it can be compared exactly and used as a map key.
const ENUM_DERIVE: &str = "#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]";

/// What the catch-all variant is called. Not `Unknown` and not `Other`: the
/// protocol uses both of those as values of its own, and a variant cannot be
/// two things at once.
const CATCH_ALL: &str = "Unrecognized";

/// The catch-all variant every generated enum ends with. Chrome ships values
/// ahead of the vendored schema, and a payload must not fail to decode over one.
const CATCH_ALL_VARIANT: &str = "\
/// A value this vendored schema does not list. Chrome adds values ahead of\n\
/// the published protocol, so an unrecognised one is kept as it arrived\n\
/// rather than failing the payload it came in.\n\
#[serde(untagged)]\n\
Unrecognized(String),";

/// Lint exemptions the generated tree needs as a whole. Lint levels follow the
/// module tree, so setting them on the parent covers every domain module.
const MODULE_ALLOWS: &str = "\
// Schema prose is reflowed to fit a doc comment, so a line that happens to\n\
// begin with a dash is not the list item this lint takes it for.\n\
#![allow(clippy::doc_lazy_continuation)]\n\
";

/// One generated item, kept beside the name it declares so that two of them
/// cannot silently claim the same one.
struct Item {
    /// The name declared, or `None` for an `impl` block, which declares none.
    name: Option<String>,
    source: String,
}

/// Indent every line of a block by one level, leaving blank lines blank.
fn indent(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            if line.is_empty() {
                line.clone()
            } else {
                format!("    {line}")
            }
        })
        .collect()
}

/// The rustdoc for one item: the protocol's own prose, then the warnings the
/// schema attaches to it.
fn item_docs(description: Option<&String>, deprecated: bool, experimental: bool) -> Vec<String> {
    let mut out = match description {
        Some(text) => wrap(&defuse(text), "///"),
        None => Vec::new(),
    };
    for (flagged, note) in [
        (deprecated, "/// Deprecated in the protocol."),
        (experimental, "/// Experimental: may change without notice."),
    ] {
        if !flagged {
            continue;
        }
        if !out.is_empty() {
            out.push("///".to_string());
        }
        out.push(note.to_string());
    }
    out
}

/// The serde attribute one field needs: its name on the wire when that differs
/// from its Rust name, and the handling that keeps an absent optional absent.
fn field_attribute(field: &Field, rust: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if rust.trim_start_matches("r#") != field.name {
        parts.push(format!("rename = \"{}\"", field.name));
    }
    if field.optional {
        // Without `skip_serializing_if` an absent optional goes out as an
        // explicit null, which CDP rejects on parameters it never declared.
        parts.push("default".to_string());
        parts.push("skip_serializing_if = \"Option::is_none\"".to_string());
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("#[serde({})]", parts.join(", ")))
}

/// Render the body of a struct: one documented, renamed field per property.
fn struct_fields(registry: &Registry, domain: &str, owner: &Owner<'_>, fields: &[Field]) -> String {
    let mut out: Vec<String> = Vec::new();
    for field in fields {
        out.extend(item_docs(
            field.description.as_ref(),
            field.deprecated,
            field.experimental,
        ));
        let rust = field_name(&field.name);
        if let Some(attribute) = field_attribute(field, &rust) {
            out.push(attribute);
        }
        out.push(format!(
            "pub {rust}: {},",
            registry.field_type(domain, owner, field)
        ));
    }
    indent(&out).join("\n")
}

/// Render a struct with the given name, docs and properties.
fn render_struct(
    registry: &Registry,
    domain: &str,
    owner: &Owner<'_>,
    name: &str,
    docs: &[String],
    fields: &[Field],
) -> Item {
    let derive = if registry.is_defaultable(domain, fields) {
        DEFAULTABLE_DERIVE
    } else {
        STRUCT_DERIVE
    };
    let mut out = docs.to_vec();
    out.push(derive.to_string());
    if fields.is_empty() {
        out.push(format!("pub struct {name} {{}}"));
    } else {
        out.push(format!("pub struct {name} {{"));
        out.push(struct_fields(registry, domain, owner, fields));
        out.push("}".to_string());
    }
    Item {
        name: Some(name.to_string()),
        source: out.join("\n"),
    }
}

/// Render a string enum, ending with the catch-all that keeps an unknown value
/// from failing the payload it arrived in.
fn render_enum(name: &str, docs: &[String], values: &[String]) -> Item {
    let mut out = docs.to_vec();
    out.push(ENUM_DERIVE.to_string());
    out.push(format!("pub enum {name} {{"));

    let mut body: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for value in values {
        let variant = variant_name(value);
        assert!(
            !seen.contains(&variant),
            "two values of enum `{name}` both become the variant `{variant}`"
        );
        assert!(
            variant != CATCH_ALL,
            "enum `{name}` has a value named `{CATCH_ALL}`, which the catch-all \
             variant already claims; rename the catch-all"
        );
        seen.push(variant.clone());

        body.push(format!("/// `{}`", defuse(value)));
        if variant != *value {
            body.push(format!("#[serde(rename = \"{value}\")]"));
        }
        body.push(format!("{variant},"));
    }
    body.extend(CATCH_ALL_VARIANT.lines().map(str::to_string));

    out.push(indent(&body).join("\n"));
    out.push("}".to_string());
    Item {
        name: Some(name.to_string()),
        source: out.join("\n"),
    }
}

/// Render an `impl` of one of the generated traits, which declares no name of
/// its own.
fn render_impl(source: String) -> Item {
    Item { name: None, source }
}

/// Render the enums for whichever of these fields state their values inline
/// instead of referring to a named type.
fn inline_enums(owner: &str, fields: &[Field]) -> Vec<Item> {
    let mut out = Vec::new();
    for field in fields {
        let Some(values) = &field.values else {
            continue;
        };
        let docs = vec![format!(
            "/// The values `{}` accepts on `{}`.",
            defuse(&field.name),
            defuse(owner)
        )];
        out.push(render_enum(
            &inline_enum_name(owner, &field.name),
            &docs,
            values,
        ));
    }
    out
}

/// Render one type the schema declares: a struct, a string enum, or an alias
/// for a shape Rust already has.
fn render_type(registry: &Registry, domain: &str, declared: &Type) -> Vec<Item> {
    let name = pascal(&declared.id);
    let docs = item_docs(
        declared.description.as_ref(),
        declared.deprecated,
        declared.experimental,
    );
    let owner = Owner::Declared(&declared.id);

    if let Some(values) = &declared.values {
        return vec![render_enum(&name, &docs, values)];
    }

    if declared.kind == "object" && !declared.properties.is_empty() {
        let mut out = vec![render_struct(
            registry,
            domain,
            &owner,
            &name,
            &docs,
            &declared.properties,
        )];
        out.extend(inline_enums(&declared.id, &declared.properties));
        return out;
    }

    // An alias states its type exactly as a field would, so it is mapped as one.
    let stated = Field {
        name: declared.id.clone(),
        description: None,
        kind: Some(declared.kind.clone()),
        reference: None,
        optional: false,
        deprecated: false,
        experimental: false,
        items: declared.items.as_ref().map(|items| Items {
            kind: items.kind.clone(),
            reference: items.reference.clone(),
        }),
        values: None,
    };
    let mut out = docs;
    out.push(format!(
        "pub type {name} = {};",
        registry.field_type(domain, &owner, &stated)
    ));
    vec![Item {
        name: Some(name),
        source: out.join("\n"),
    }]
}

/// Render one command: its parameters, its result, and the tie between them.
fn render_command(registry: &Registry, domain: &str, command: &Command) -> Vec<Item> {
    let base = pascal(&command.name);
    let params = format!("{base}Params");
    let owner = Owner::Generated(&command.name);
    let method = format!("{}.{}", domain, command.name);

    let mut docs = item_docs(
        command.description.as_ref(),
        command.deprecated,
        command.experimental,
    );
    if !docs.is_empty() {
        docs.push("///".to_string());
    }
    docs.push(format!("/// Parameters of `{method}`."));

    let mut out = vec![render_struct(
        registry,
        domain,
        &owner,
        &params,
        &docs,
        &command.parameters,
    )];
    out.extend(inline_enums(&command.name, &command.parameters));

    let returns = if command.returns.is_empty() {
        "crate::typed::NoReturns".to_string()
    } else {
        let name = format!("{base}Returns");
        let docs = vec![format!("/// What `{method}` answers with.")];
        out.push(render_struct(
            registry,
            domain,
            &owner,
            &name,
            &docs,
            &command.returns,
        ));
        out.extend(inline_enums(&command.name, &command.returns));
        name
    };

    out.push(render_impl(format!(
        "impl crate::typed::Command for {params} {{\n\
         \x20   const METHOD: &'static str = \"{method}\";\n\
         \x20   type Returns = {returns};\n\
         }}",
    )));
    out
}

/// Render one event: its payload and the method name that identifies it.
fn render_event(registry: &Registry, domain: &str, event: &Event) -> Vec<Item> {
    let name = format!("{}Event", pascal(&event.name));
    let owner = Owner::Generated(&event.name);
    let method = format!("{}.{}", domain, event.name);

    let mut docs = item_docs(
        event.description.as_ref(),
        event.deprecated,
        event.experimental,
    );
    if !docs.is_empty() {
        docs.push("///".to_string());
    }
    docs.push(format!("/// Payload of the `{method}` event."));

    let mut out = vec![render_struct(
        registry,
        domain,
        &owner,
        &name,
        &docs,
        &event.parameters,
    )];
    out.extend(inline_enums(&event.name, &event.parameters));
    out.push(render_impl(format!(
        "impl crate::typed::Event for {name} {{\n\
         \x20   const METHOD: &'static str = \"{method}\";\n\
         }}",
    )));
    out
}

/// The module header: the domain's own description and what the module holds.
fn module_docs(domain: &Domain) -> String {
    let mut out = String::new();
    match &domain.description {
        Some(text) => {
            out.push_str(&wrap(&defuse(text), "//!").join("\n"));
            out.push('\n');
        }
        None => out.push_str(&format!("//! The `{}` domain.\n", domain.domain)),
    }
    out.push_str(&format!(
        "//!\n//! Types, command parameters and event payloads for the `{}`\n\
         //! domain. The method-name constants for the same domain are in\n\
         //! [`crate::methods`].\n",
        domain.domain
    ));
    if domain.deprecated {
        out.push_str("//!\n//! This domain is deprecated in the protocol.\n");
    }
    if domain.experimental {
        out.push_str("//!\n//! This domain is experimental and may change without notice.\n");
    }
    out
}

/// Render one domain as a module of types, command parameters and events.
///
/// Items keep the schema's own order, so regenerating an unchanged schema moves
/// nothing.
pub fn domain(registry: &Registry, domain: &Domain) -> String {
    let mut items: Vec<Item> = Vec::new();
    for declared in &domain.types {
        items.extend(render_type(registry, &domain.domain, declared));
    }
    for command in &domain.commands {
        items.extend(render_command(registry, &domain.domain, command));
    }
    for event in &domain.events {
        items.extend(render_event(registry, &domain.domain, event));
    }

    let mut out = String::new();
    out.push_str(BANNER);
    out.push('\n');
    out.push_str(&module_docs(domain));
    out.push_str("\nuse serde::{Deserialize, Serialize};\n");

    let mut declared: Vec<&str> = Vec::new();
    for item in &items {
        if let Some(name) = &item.name {
            assert!(
                !declared.contains(&name.as_str()),
                "domain `{}` would declare `{name}` twice",
                domain.domain
            );
            declared.push(name);
        }
        out.push('\n');
        out.push_str(&item.source);
        out.push('\n');
    }
    out
}

/// Render the parent module that publishes every domain module.
pub fn index(modules: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    out.push_str(BANNER);
    out.push_str(
        "\n//! Typed CDP: the protocol's own types, one parameters and one result\n\
         //! struct per command, and one payload struct per event.\n\
         //!\n\
         //! Send a command with [`CdpClient::send`](crate::CdpClient::send), which\n\
         //! reads the method name and the result type off the parameters it is\n\
         //! handed, so the two cannot be mismatched. Decode an event with\n\
         //! [`typed::decode`](crate::typed::decode) or\n\
         //! [`SessionEvent::decode`](crate::SessionEvent::decode).\n\
         //!\n\
         //! ```no_run\n\
         //! # use cdp_driver::CdpClient;\n\
         //! use cdp_driver::protocol::target;\n\
         //!\n\
         //! # async fn run(client: &CdpClient) -> cdp_driver::Result<()> {\n\
         //! let created = client\n\
         //!     .send(target::CreateTargetParams {\n\
         //!         url: \"about:blank\".to_string(),\n\
         //!         ..Default::default()\n\
         //!     })\n\
         //!     .await?;\n\
         //! println!(\"{}\", created.target_id);\n\
         //! # Ok(())\n\
         //! # }\n\
         //! ```\n\n",
    );
    out.push_str(MODULE_ALLOWS);
    out.push('\n');
    for module in modules.keys() {
        out.push_str(&format!("pub mod {module};\n"));
    }
    out
}
