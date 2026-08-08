//! Turning schema prose into rustdoc.
//!
//! Descriptions in the protocol were never written to be rustdoc: they contain
//! brackets, angle brackets and bare URLs, each of which rustdoc reads as markup
//! and warns about. Everything here exists to hand those over safely.

/// Doc comments are wrapped to this width to keep generated files readable.
const DOC_WIDTH: usize = 74;

/// Escape the markdown and HTML that rustdoc would otherwise interpret.
fn escape(text: &str) -> String {
    text.replace('[', "\\[")
        .replace(']', "\\]")
        .replace('<', "\\<")
        .replace('>', "\\>")
}

/// Where the URL at the front of `text` ends.
///
/// Stops at a bracket or quote that closes around the URL rather than belonging
/// to it, then backs off trailing sentence punctuation.
fn url_end(text: &str) -> usize {
    let hard_stop = text
        .find([')', ']', '}', '>', '"', '\'', ','])
        .unwrap_or(text.len());
    text[..hard_stop].trim_end_matches(['.', ';', ':']).len()
}

/// Rewrite one word, wrapping any bare URL inside it in angle brackets so
/// rustdoc links it instead of warning, and escaping everything around it.
///
/// A URL is not always at the front of a word: the protocol contains prose like
/// `TODO(https://crbug.com/1440085):`, so the scan is positional.
fn defuse_token(token: &str) -> String {
    let Some(start) = ["https://", "http://"]
        .iter()
        .filter_map(|scheme| token.find(scheme))
        .min()
    else {
        return escape(token);
    };

    let end = start + url_end(&token[start..]);
    format!(
        "{}<{}>{}",
        escape(&token[..start]),
        &token[start..end],
        defuse_token(&token[end..])
    )
}

/// Prepare schema prose for use as rustdoc, collapsing whitespace along the way.
pub fn defuse(text: &str) -> String {
    text.split_whitespace()
        .map(defuse_token)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Wrap prose to [`DOC_WIDTH`], emitting each line with the given comment
/// marker (`///` for items, `//!` for modules).
pub fn wrap(text: &str, marker: &str) -> Vec<String> {
    let flattened = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flattened.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut line = String::new();
    for word in flattened.split(' ') {
        if !line.is_empty() && line.len() + 1 + word.len() > DOC_WIDTH {
            lines.push(format!("{marker} {line}"));
            line = String::new();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(format!("{marker} {line}"));
    }
    lines
}
