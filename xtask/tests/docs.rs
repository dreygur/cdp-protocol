//! What the schema-prose-to-rustdoc conversion promises.
//!
//! Every case here is drawn from prose that actually appears in the CDP
//! schemas and that rustdoc would otherwise warn about.

use xtask::docs::{defuse, wrap};

#[test]
fn square_brackets_are_escaped_so_rustdoc_sees_no_link() {
    assert_eq!(
        defuse("see [Runtime.evaluate]"),
        "see \\[Runtime.evaluate\\]"
    );
}

#[test]
fn angle_brackets_are_escaped_so_rustdoc_sees_no_html() {
    assert_eq!(
        defuse("wraps the <video> element"),
        "wraps the \\<video\\> element"
    );
}

#[test]
fn ordinary_prose_passes_through_unchanged() {
    assert_eq!(defuse("Creates a new page."), "Creates a new page.");
}

#[test]
fn runs_of_whitespace_collapse() {
    assert_eq!(defuse("one\n  two\tthree"), "one two three");
}

#[test]
fn a_bare_url_is_wrapped_so_rustdoc_links_it() {
    assert_eq!(
        defuse("see https://crbug.com/1440085"),
        "see <https://crbug.com/1440085>"
    );
}

#[test]
fn a_trailing_period_stays_outside_the_link() {
    assert_eq!(
        defuse("solution for https://crbug.com/1440085."),
        "solution for <https://crbug.com/1440085>."
    );
}

#[test]
fn a_url_inside_parentheses_is_still_found() {
    // Real prose from Page.setPrerenderingAllowed.
    assert_eq!(
        defuse("TODO(https://crbug.com/1440085): Remove"),
        "TODO(<https://crbug.com/1440085>): Remove"
    );
}

#[test]
fn a_fragment_url_keeps_its_hash() {
    assert_eq!(
        defuse("https://w3c.github.io/spec/#rph-automation"),
        "<https://w3c.github.io/spec/#rph-automation>"
    );
}

#[test]
fn plain_http_is_recognised_too() {
    assert_eq!(defuse("http://example.com"), "<http://example.com>");
}

#[test]
fn short_prose_stays_on_one_line() {
    assert_eq!(
        wrap("Creates a new page.", "///"),
        ["/// Creates a new page."]
    );
}

#[test]
fn long_prose_wraps_and_every_line_carries_the_marker() {
    let long = "word ".repeat(40);
    let lines = wrap(&long, "//!");

    assert!(lines.len() > 1, "40 words must not fit on one line");
    for line in &lines {
        assert!(
            line.starts_with("//! "),
            "every line needs a marker: {line}"
        );
        assert!(line.len() <= 78, "line ran long: {line}");
    }
}

#[test]
fn empty_prose_yields_no_lines() {
    assert!(wrap("", "///").is_empty());
    assert!(wrap("   \n  ", "///").is_empty());
}
