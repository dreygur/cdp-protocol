//! Connection and viewport settings for [`BrowserAgent::connect_with_config`](crate::agent::BrowserAgent::connect_with_config).

/// Connection and viewport settings shared by [`BrowserAgent`](crate::agent::BrowserAgent)
/// and [`Cluster`](crate::cluster::Cluster).
#[derive(Debug, Clone)]
pub struct Config {
    /// Chrome's remote-debugging host, e.g. `"localhost"`.
    pub host: String,
    /// Chrome's remote-debugging port, e.g. `9222`.
    pub port: u16,
    /// Viewport width in CSS pixels applied on connect.
    pub viewport_width: i32,
    /// Viewport height in CSS pixels applied on connect.
    pub viewport_height: i32,
    /// Directory screenshots are written to when a relative path is used.
    pub screenshots_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            host: "localhost".into(),
            port: 9222,
            viewport_width: 1920,
            viewport_height: 1200,
            screenshots_dir: "screenshots".into(),
        }
    }
}
