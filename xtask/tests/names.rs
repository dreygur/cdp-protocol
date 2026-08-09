//! What the CDP-identifier-to-Rust-name conversion promises.
//!
//! Acronym runs are the whole difficulty: a per-capital split turns
//! `getDOMCounters` into `GET_D_O_M_COUNTERS`, which is why these exist.

use xtask::names::{pascal, screaming_snake, snake, words};

#[test]
fn a_plain_camel_case_name_splits_on_each_capital() {
    assert_eq!(words("createTarget"), ["create", "target"]);
    assert_eq!(screaming_snake("createTarget"), "CREATE_TARGET");
}

#[test]
fn an_acronym_run_stays_one_word() {
    assert_eq!(words("getDOMCounters"), ["get", "dom", "counters"]);
    assert_eq!(screaming_snake("getDOMCounters"), "GET_DOM_COUNTERS");
}

#[test]
fn an_acronym_at_the_end_stays_one_word() {
    assert_eq!(words("printToPDF"), ["print", "to", "pdf"]);
    assert_eq!(screaming_snake("printToPDF"), "PRINT_TO_PDF");
}

#[test]
fn an_acronym_in_the_middle_keeps_the_following_word() {
    assert_eq!(words("getFullAXTree"), ["get", "full", "ax", "tree"]);
    assert_eq!(screaming_snake("getFullAXTree"), "GET_FULL_AX_TREE");
    assert_eq!(screaming_snake("getOuterHTML"), "GET_OUTER_HTML");
}

#[test]
fn a_domain_that_is_only_an_acronym_becomes_one_word() {
    assert_eq!(snake("CSS"), "css");
    assert_eq!(snake("IO"), "io");
    assert_eq!(snake("PWA"), "pwa");
}

#[test]
fn a_domain_that_opens_with_an_acronym_splits_after_it() {
    assert_eq!(snake("DOMSnapshot"), "dom_snapshot");
    assert_eq!(snake("DOMDebugger"), "dom_debugger");
    assert_eq!(snake("IndexedDB"), "indexed_db");
    assert_eq!(snake("WebMCP"), "web_mcp");
}

#[test]
fn a_digit_opens_the_next_word() {
    assert_eq!(snake("Log4Shell"), "log4_shell");
}

#[test]
fn an_empty_name_yields_no_words() {
    assert!(words("").is_empty());
    assert_eq!(snake(""), "");
}

#[test]
fn a_type_name_keeps_its_shape_in_pascal_case() {
    assert_eq!(pascal("createTarget"), "CreateTarget");
    assert_eq!(pascal("Node"), "Node");
    assert_eq!(pascal("getOuterHTML"), "GetOuterHtml");
}

#[test]
fn punctuation_in_an_enum_value_breaks_a_word() {
    assert_eq!(words("font-face"), ["font", "face"]);
    assert_eq!(pascal("font-face"), "FontFace");
    assert_eq!(pascal("auto_bookmark"), "AutoBookmark");
    assert_eq!(pascal("ch-ua-full-version-list"), "ChUaFullVersionList");
}

#[test]
fn enum_values_that_differ_only_in_a_digit_stay_distinct() {
    assert_eq!(pascal("ctap2_0"), "Ctap20");
    assert_eq!(pascal("ctap2_1"), "Ctap21");
}

#[test]
fn a_mixed_case_enum_value_keeps_its_word_boundaries() {
    assert_eq!(
        pascal("RenderFrameHostReused_CrossSite"),
        "RenderFrameHostReusedCrossSite"
    );
}
