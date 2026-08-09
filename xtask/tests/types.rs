//! What the CDP-type-to-Rust-type mapping promises.
//!
//! The fields under test are written as the schema writes them and parsed with
//! the same reader the generator uses, so a test cannot drift into describing a
//! shape the protocol never states.

use serde_json::json;

use xtask::schema::{self, Field};
use xtask::types::{field_name, inline_enum_name, registry, Owner, Registry};
use xtask::{repository_root, types};

/// Every mapping question is asked against the real vendored schema, so that a
/// reference in a test is one the protocol actually declares.
fn vendored() -> Registry {
    registry(&schema::load(&repository_root()))
}

/// Parse one field the way the schema states it.
fn field(stated: serde_json::Value) -> Field {
    serde_json::from_value(stated).expect("a field the schema could have stated")
}

/// The Rust type of a field belonging to a type the schema declares.
fn declared(domain: &str, owner: &str, stated: serde_json::Value) -> String {
    vendored().field_type(domain, &Owner::Declared(owner), &field(stated))
}

#[test]
fn a_primitive_maps_to_its_rust_counterpart() {
    let registry = vendored();
    let owner = Owner::Generated("someCommand");
    let mapping = [
        ("string", "String"),
        ("integer", "i64"),
        ("number", "f64"),
        ("boolean", "bool"),
        ("object", "serde_json::Value"),
        ("any", "serde_json::Value"),
    ];
    for (cdp, rust) in mapping {
        let stated = field(json!({ "name": "value", "type": cdp }));
        assert_eq!(registry.field_type("DOM", &owner, &stated), rust);
    }
}

#[test]
fn an_optional_field_is_wrapped_in_option() {
    assert_eq!(
        declared(
            "DOM",
            "Node",
            json!({ "name": "value", "type": "string", "optional": true })
        ),
        "Option<String>"
    );
}

#[test]
fn a_bare_reference_stays_inside_its_own_domain() {
    assert_eq!(
        declared(
            "DOM",
            "BoxModel",
            json!({ "name": "nodeId", "$ref": "NodeId" })
        ),
        "NodeId"
    );
}

#[test]
fn a_qualified_reference_becomes_a_path_to_the_other_domain() {
    assert_eq!(
        declared(
            "Page",
            "Frame",
            json!({ "name": "loaderId", "$ref": "Network.LoaderId" })
        ),
        "crate::protocol::network::LoaderId"
    );
}

#[test]
fn an_array_takes_the_type_of_its_elements() {
    assert_eq!(
        declared(
            "DOM",
            "Node",
            json!({ "name": "quads", "type": "array", "items": { "$ref": "Quad" } })
        ),
        "Vec<Quad>"
    );
    assert_eq!(
        declared(
            "DOM",
            "Node",
            json!({ "name": "names", "type": "array", "items": { "type": "string" } })
        ),
        "Vec<String>"
    );
}

#[test]
fn a_type_that_contains_itself_is_boxed() {
    assert_eq!(
        declared(
            "DOM",
            "Node",
            json!({ "name": "contentDocument", "$ref": "Node", "optional": true })
        ),
        "Option<Box<Node>>"
    );
    assert_eq!(
        declared(
            "Runtime",
            "StackTrace",
            json!({ "name": "parent", "$ref": "StackTrace", "optional": true })
        ),
        "Option<Box<StackTrace>>"
    );
}

#[test]
fn a_reference_that_cannot_come_back_is_not_boxed() {
    assert_eq!(
        declared("DOM", "Node", json!({ "name": "nodeId", "$ref": "NodeId" })),
        "NodeId"
    );
}

#[test]
fn an_array_of_the_owners_own_type_needs_no_box() {
    assert_eq!(
        declared(
            "DOM",
            "Node",
            json!({ "name": "children", "type": "array", "items": { "$ref": "Node" }, "optional": true })
        ),
        "Option<Vec<Node>>"
    );
}

#[test]
fn a_field_that_lists_its_values_inline_gets_an_enum_of_its_own() {
    assert_eq!(
        declared(
            "DOM",
            "enable",
            json!({ "name": "includeWhitespace", "type": "string", "enum": ["none", "all"], "optional": true })
        ),
        "Option<EnableIncludeWhitespace>"
    );
    assert_eq!(inline_enum_name("CSSAtRule", "type"), "CssAtRuleType");
}

#[test]
fn a_field_named_after_a_keyword_is_escaped() {
    assert_eq!(field_name("type"), "r#type");
    assert_eq!(field_name("override"), "r#override");
    assert_eq!(field_name("self"), "self_");
    assert_eq!(field_name("backendNodeId"), "backend_node_id");
}

#[test]
#[should_panic(expected = "refers to a type no domain declares")]
fn a_reference_to_nothing_stops_the_generator() {
    declared(
        "DOM",
        "Node",
        json!({ "name": "ghost", "$ref": "NoSuchType" }),
    );
}

#[test]
fn every_reference_in_the_vendored_schema_resolves() {
    let domains = schema::load(&repository_root());
    let registry = types::registry(&domains);

    for domain in &domains {
        for declaration in &domain.types {
            for property in &declaration.properties {
                registry.field_type(&domain.domain, &Owner::Declared(&declaration.id), property);
            }
        }
        for command in &domain.commands {
            let owner = Owner::Generated(&command.name);
            for property in command.parameters.iter().chain(&command.returns) {
                registry.field_type(&domain.domain, &owner, property);
            }
        }
        for event in &domain.events {
            let owner = Owner::Generated(&event.name);
            for property in &event.parameters {
                registry.field_type(&domain.domain, &owner, property);
            }
        }
    }
}
