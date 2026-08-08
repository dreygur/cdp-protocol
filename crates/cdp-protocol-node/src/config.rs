//! Connection and viewport settings, as a plain JS object.

use cdp_driver::Config as CoreConfig;
use napi_derive::napi;

/// Connection / viewport configuration.
#[napi(object)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub viewport_width: i32,
    pub viewport_height: i32,
}

impl Default for Config {
    fn default() -> Self {
        let c = CoreConfig::default();
        Config {
            host: c.host,
            port: c.port,
            viewport_width: c.viewport_width,
            viewport_height: c.viewport_height,
        }
    }
}

impl From<Config> for CoreConfig {
    fn from(c: Config) -> Self {
        CoreConfig {
            host: c.host,
            port: c.port,
            viewport_width: c.viewport_width,
            viewport_height: c.viewport_height,
            ..CoreConfig::default()
        }
    }
}
