//! Turning CDP identifiers into Rust ones.
//!
//! The protocol is camelCase with acronym runs left intact, so `getDOMCounters`
//! and `printToPDF` both need splitting in a way that a per-capital scan gets
//! wrong.

/// Split a CDP identifier into lowercase words, keeping acronym runs whole.
///
/// `getDOMCounters` becomes `[get, dom, counters]`, not `[get, d, o, m, ...]`.
pub fn words(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();

    for (i, &ch) in chars.iter().enumerate() {
        // A capital after a lowercase always opens a word. A capital inside an
        // acronym opens one only when a lowercase follows, so that the run in
        // `DOMCounters` breaks as `DOM` + `Counters`.
        let opens_word = ch.is_uppercase()
            && i > 0
            && (chars[i - 1].is_lowercase()
                || chars[i - 1].is_numeric()
                || chars.get(i + 1).is_some_and(|next| next.is_lowercase()));

        if opens_word && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        current.push(ch.to_ascii_lowercase());
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// `getDOMCounters` -> `GET_DOM_COUNTERS`, for constant names.
pub fn screaming_snake(name: &str) -> String {
    words(name).join("_").to_uppercase()
}

/// `DOMSnapshot` -> `dom_snapshot`, for module names.
pub fn snake(name: &str) -> String {
    words(name).join("_")
}
