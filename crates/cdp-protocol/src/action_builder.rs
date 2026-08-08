//! Assembling a sequence of [`BrowserAction`]s to run in one go.

use crate::action::BrowserAction;

/// Fluent builder for a sequence of [`BrowserAction`]s, run with
/// [`BrowserAgent::execute_many`](crate::agent::BrowserAgent::execute_many).
pub struct ActionBuilder {
    actions: Vec<BrowserAction>,
}

impl Default for ActionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionBuilder {
    /// Start an empty action sequence.
    pub fn new() -> Self {
        ActionBuilder {
            actions: Vec::new(),
        }
    }

    /// Append [`BrowserAction::Navigate`].
    pub fn navigate(mut self, url: &str) -> Self {
        self.actions
            .push(BrowserAction::Navigate { url: url.into() });
        self
    }

    /// Append [`BrowserAction::Wait`].
    pub fn wait(mut self, ms: u64) -> Self {
        self.actions.push(BrowserAction::Wait { ms });
        self
    }

    /// Append [`BrowserAction::Click`] targeting a CSS selector.
    pub fn click(mut self, selector: &str) -> Self {
        self.actions.push(BrowserAction::Click {
            selector: Some(selector.into()),
            x: None,
            y: None,
        });
        self
    }

    /// Append [`BrowserAction::Fill`].
    pub fn fill(mut self, selector: &str, value: &str) -> Self {
        self.actions.push(BrowserAction::Fill {
            selector: selector.into(),
            value: value.into(),
        });
        self
    }

    /// Append [`BrowserAction::PressKey`].
    pub fn press_key(mut self, key: &str) -> Self {
        self.actions
            .push(BrowserAction::PressKey { key: key.into() });
        self
    }

    /// Append [`BrowserAction::Screenshot`].
    pub fn screenshot(mut self, path: Option<&str>) -> Self {
        self.actions.push(BrowserAction::Screenshot {
            path: path.map(Into::into),
        });
        self
    }

    /// Append [`BrowserAction::Evaluate`].
    pub fn evaluate(mut self, expr: &str) -> Self {
        self.actions.push(BrowserAction::Evaluate {
            expression: expr.into(),
        });
        self
    }

    /// Append [`BrowserAction::Scroll`].
    pub fn scroll(mut self, x: f64, y: f64) -> Self {
        self.actions.push(BrowserAction::Scroll { x, y });
        self
    }

    /// Append [`BrowserAction::GetTitle`].
    pub fn get_title(mut self) -> Self {
        self.actions.push(BrowserAction::GetTitle);
        self
    }

    /// Finish building and return the action sequence.
    pub fn build(self) -> Vec<BrowserAction> {
        self.actions
    }
}
