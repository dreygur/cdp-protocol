use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// Initialise tracing from RUST_LOG. Returns a guard that must be held
/// for the lifetime of the process — dropping it flushes and shuts down
/// the non-blocking writer.
///
/// Set RUST_LOG_SYNC=1 to use synchronous (blocking) output instead.
pub fn init() -> Option<WorkerGuard> {
    let filter = EnvFilter::from_default_env();

    if std::env::var("RUST_LOG_SYNC").is_ok() {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .init();
        None
    } else {
        let (writer, guard) = tracing_appender::non_blocking(std::io::stdout());
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(writer)
            .init();
        Some(guard)
    }
}
