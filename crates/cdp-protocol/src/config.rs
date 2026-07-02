#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub viewport_width: i32,
    pub viewport_height: i32,
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
