//! Keyboard names as `Input.dispatchKeyEvent` wants them.

/// The `code` and Windows virtual key code for a key name.
///
/// Unrecognised keys pass through with a virtual key code of 0, which is what
/// CDP expects for printable characters that carry no special code.
pub fn key_info(key: &str) -> (&str, u32) {
    match key {
        "Enter" => ("Enter", 13),
        "Tab" => ("Tab", 9),
        "Backspace" => ("Backspace", 8),
        "Delete" => ("Delete", 46),
        "Escape" => ("Escape", 27),
        " " | "Space" => ("Space", 32),
        "ArrowLeft" => ("ArrowLeft", 37),
        "ArrowUp" => ("ArrowUp", 38),
        "ArrowRight" => ("ArrowRight", 39),
        "ArrowDown" => ("ArrowDown", 40),
        _ => (key, 0),
    }
}
