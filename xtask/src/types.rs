//! Mapping CDP's type language onto Rust's.
//!
//! The schema states a type as a primitive name, an array with an element type,
//! or a `$ref` to a named type that may live in another domain. Turning that
//! into a Rust type needs three things the schema does not say outright: where a
//! cross-domain reference ends up as a module path, which references form a
//! cycle and so have to be boxed, and which field names Rust refuses.

use std::collections::{BTreeMap, BTreeSet};

use crate::names::{pascal, snake};
use crate::schema::{Domain, Field, Items, Type};

/// How each CDP primitive is spelled in Rust. `object` carries no declared shape
/// where it is used as a primitive, and `any` never does, so both become
/// free-form JSON rather than a struct with nothing in it.
const PRIMITIVES: [(&str, &str); 6] = [
    ("string", "String"),
    ("integer", "i64"),
    ("number", "f64"),
    ("boolean", "bool"),
    ("object", "serde_json::Value"),
    ("any", "serde_json::Value"),
];

/// Where the generated modules live, for spelling a cross-domain reference.
const MODULE_PATH: &str = "crate::protocol";

/// Words Rust will not accept as a plain identifier. Most survive as a raw
/// identifier; the ones in [`UNRAWABLE`] do not.
const KEYWORDS: [&str; 51] = [
    "as", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "async", "await", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen",
];

/// Keywords that cannot be written `r#like_this`, so a field named one of them
/// takes a trailing underscore instead.
const UNRAWABLE: [&str; 4] = ["crate", "self", "Self", "super"];

/// A named type, qualified by the domain that declares it.
pub type Key = (String, String);

/// What a field belongs to.
///
/// Only a type the protocol declares can take part in a reference cycle, since
/// nothing in the schema can refer to a generated params or event struct.
pub enum Owner<'a> {
    /// A type the schema declares under `types`.
    Declared(&'a str),
    /// A struct this generator invents for a command or an event.
    Generated(&'a str),
}

/// Which named types exist, and which of them can contain themselves.
pub struct Registry {
    /// Every declared type, so a `$ref` is checked rather than trusted.
    declared: BTreeSet<Key>,
    /// Containment edges an array does not break: `a -> b` means a value of `a`
    /// holds a value of `b` inline.
    contains: BTreeMap<Key, Vec<Key>>,
    /// The types that can derive `Default`, which is every type that does not
    /// require a string enum somewhere inside it.
    defaultable: BTreeSet<Key>,
}

impl Owner<'_> {
    /// The schema's name for this owner, which prefixes any inline enum it
    /// declares.
    pub fn name(&self) -> &str {
        match self {
            Owner::Declared(name) | Owner::Generated(name) => name,
        }
    }
}

/// Resolve a `$ref`. It is bare within its own domain and qualified with a
/// domain name across domains.
fn resolve(domain: &str, reference: &str) -> Key {
    match reference.split_once('.') {
        Some((other, id)) => (other.to_string(), id.to_string()),
        None => (domain.to_string(), reference.to_string()),
    }
}

/// Whether `target` is reachable from `from` by inline containment, which is
/// what would make a field holding `target` infinitely sized.
fn reaches(contains: &BTreeMap<Key, Vec<Key>>, from: &Key, target: &Key) -> bool {
    let mut seen: BTreeSet<Key> = BTreeSet::new();
    let mut stack = vec![from.clone()];
    while let Some(key) = stack.pop() {
        if &key == target {
            return true;
        }
        if !seen.insert(key.clone()) {
            continue;
        }
        stack.extend(contains.get(&key).into_iter().flatten().cloned());
    }
    false
}

/// Make an identifier Rust will accept, out of one it might not.
fn escaped(name: String) -> String {
    if UNRAWABLE.contains(&name.as_str()) {
        return format!("{name}_");
    }
    if KEYWORDS.contains(&name.as_str()) {
        return format!("r#{name}");
    }
    name
}

/// The Rust field name for a schema property, escaped if Rust claims the word.
pub fn field_name(name: &str) -> String {
    escaped(snake(name))
}

/// The Rust variant name for one value of a string enum. The protocol has a
/// value spelled `Self`, which Rust will not accept even as a raw identifier.
pub fn variant_name(value: &str) -> String {
    escaped(pascal(value))
}

/// The name of the enum generated for a field that lists its values inline
/// rather than referring to a named type.
pub fn inline_enum_name(owner: &str, field: &str) -> String {
    format!("{}{}", pascal(owner), pascal(field))
}

/// The Rust path to a named type, bare within its own domain.
fn path(domain: &str, key: &Key) -> String {
    if key.0 == domain {
        return pascal(&key.1);
    }
    format!("{MODULE_PATH}::{}::{}", snake(&key.0), pascal(&key.1))
}

/// The Rust spelling of a CDP primitive.
fn primitive(kind: &str) -> String {
    PRIMITIVES
        .iter()
        .find(|(cdp, _)| *cdp == kind)
        .map(|(_, rust)| (*rust).to_string())
        .unwrap_or_else(|| panic!("no Rust type for the CDP primitive `{kind}`"))
}

/// Resolve a reference, insisting the target exists. A dangling `$ref` would
/// otherwise become a Rust path to nothing, a hundred generated files later.
fn checked(registry: &Registry, domain: &str, reference: &str) -> Key {
    let key = resolve(domain, reference);
    assert!(
        registry.declared.contains(&key),
        "`{reference}` in domain `{domain}` refers to a type no domain declares"
    );
    key
}

/// The Rust type of an array's elements.
fn items_type(registry: &Registry, domain: &str, items: Option<&Items>) -> String {
    let Some(items) = items else {
        // An array with no `items` says nothing about what is in it.
        return "serde_json::Value".to_string();
    };
    if let Some(reference) = &items.reference {
        return path(domain, &checked(registry, domain, reference));
    }
    primitive(items.kind.as_deref().unwrap_or("any"))
}

/// A field's type, ignoring optionality and boxing.
fn base_type(registry: &Registry, domain: &str, owner: &Owner<'_>, field: &Field) -> String {
    if field.values.is_some() {
        return inline_enum_name(owner.name(), &field.name);
    }
    if let Some(reference) = &field.reference {
        return path(domain, &checked(registry, domain, reference));
    }
    match field.kind.as_deref() {
        Some("array") => format!(
            "Vec<{}>",
            items_type(registry, domain, field.items.as_ref())
        ),
        Some(kind) => primitive(kind),
        None => panic!("field `{}` states no type at all", field.name),
    }
}

/// Whether one type can derive `Default`, answering for everything it requires
/// along the way.
///
/// A string enum cannot: no value of it is a sensible default, and the protocol
/// does not nominate one. Anything that requires such an enum inherits the
/// answer. A cycle cannot bottom out, so it counts as no.
fn defaultable(
    index: &BTreeMap<Key, &Type>,
    known: &mut BTreeMap<Key, bool>,
    asking: &mut Vec<Key>,
    key: &Key,
) -> bool {
    if let Some(answer) = known.get(key) {
        return *answer;
    }
    if asking.contains(key) {
        return false;
    }
    // A reference that resolves to nothing is caught, with a better message,
    // when the field that holds it is rendered.
    let Some(declared) = index.get(key) else {
        return true;
    };
    if declared.values.is_some() {
        known.insert(key.clone(), false);
        return false;
    }

    asking.push(key.clone());
    let answer = declared.properties.iter().all(|field| {
        if field.optional {
            return true;
        }
        if field.values.is_some() {
            return false;
        }
        match &field.reference {
            Some(reference) => defaultable(index, known, asking, &resolve(&key.0, reference)),
            None => true,
        }
    });
    asking.pop();

    known.insert(key.clone(), answer);
    answer
}

impl Registry {
    /// Whether a field holding `key` inline would make `owner` infinitely sized.
    fn is_cyclic(&self, owner: &Key, key: &Key) -> bool {
        reaches(&self.contains, key, owner)
    }

    /// Whether a struct with these fields can derive `Default`, which is what
    /// lets a caller name the parameters it cares about and write
    /// `..Default::default()` for the rest.
    pub fn is_defaultable(&self, domain: &str, fields: &[Field]) -> bool {
        fields.iter().all(|field| {
            if field.optional {
                return true;
            }
            if field.values.is_some() {
                return false;
            }
            match &field.reference {
                Some(reference) => self.defaultable.contains(&resolve(domain, reference)),
                None => true,
            }
        })
    }

    /// The Rust type to write for one field: boxed where a cycle demands it, and
    /// wrapped in `Option` where the schema calls the field optional.
    pub fn field_type(&self, domain: &str, owner: &Owner<'_>, field: &Field) -> String {
        let mut rust = base_type(self, domain, owner, field);

        // Only a direct reference can cycle: `Vec` already puts its elements
        // behind a pointer, so an array of the owner's own type is finitely
        // sized without help.
        if let (Owner::Declared(name), Some(reference)) = (owner, &field.reference) {
            let owner_key = (domain.to_string(), (*name).to_string());
            let target = checked(self, domain, reference);
            if self.is_cyclic(&owner_key, &target) {
                rust = format!("Box<{rust}>");
            }
        }

        if field.optional {
            rust = format!("Option<{rust}>");
        }
        rust
    }
}

/// Index every domain's types, recording what each one contains inline and
/// which of them can be defaulted.
pub fn registry(domains: &[Domain]) -> Registry {
    let mut declared = BTreeSet::new();
    let mut contains: BTreeMap<Key, Vec<Key>> = BTreeMap::new();
    let mut index: BTreeMap<Key, &Type> = BTreeMap::new();

    for domain in domains {
        for declaration in &domain.types {
            let key = (domain.domain.clone(), declaration.id.clone());
            declared.insert(key.clone());
            index.insert(key.clone(), declaration);

            let inline = declaration
                .properties
                .iter()
                .filter(|field| field.kind.as_deref() != Some("array"))
                .filter_map(|field| field.reference.as_deref())
                .map(|reference| resolve(&domain.domain, reference))
                .collect();
            contains.insert(key, inline);
        }
    }

    let mut known: BTreeMap<Key, bool> = BTreeMap::new();
    for key in &declared {
        defaultable(&index, &mut known, &mut Vec::new(), key);
    }
    let defaultable = known
        .into_iter()
        .filter(|(_, answer)| *answer)
        .map(|(key, _)| key)
        .collect();

    Registry {
        declared,
        contains,
        defaultable,
    }
}
