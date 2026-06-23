//! The `useragent` VGI worker.
//!
//! A standalone binary that DuckDB launches and talks to over Apache Arrow IPC
//! (`ATTACH 'useragent' (TYPE vgi, LOCATION '…')`). It parses HTTP User-Agent
//! strings into browser / OS / device under the catalog `useragent`, schema
//! `main`:
//!
//! ```sql
//! ATTACH 'useragent' (TYPE vgi, LOCATION './target/release/useragent-worker');
//! SET search_path = 'useragent.main';
//!
//! SELECT ua_browser(ua), ua_os(ua), ua_device(ua) FROM hits;
//! SELECT ua_is_bot('Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)');
//! SELECT (ua_parse(ua)).*  FROM hits;   -- browser/os/device/brand/is_bot
//! ```
//!
//! Pure parsing logic (wrapping the `uaparser`/uap-core regex database, embedded
//! at compile time) lives in `useragent.rs`; the `scalar/` modules are thin Arrow
//! adapters over it. The parser is compiled exactly once per process.

mod arrow_io;
mod scalar;
mod useragent;

use vgi::Worker;

/// Worker version string, surfaced by `useragent_version()`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn main() {
    // Logs MUST go to stderr — stdout is the Arrow-IPC channel.
    let _ = env_logger::Builder::from_env(env_logger::Env::default().filter_or("VGI_LOG", "info"))
        .format_timestamp_millis()
        .try_init();

    // The catalog name DuckDB sees in `ATTACH 'useragent' (TYPE vgi, …)`. Default
    // to `useragent`, but honor an explicit override so a test harness can rename.
    if std::env::var_os("VGI_WORKER_CATALOG_NAME").is_none() {
        std::env::set_var("VGI_WORKER_CATALOG_NAME", "useragent");
    }

    let mut worker = Worker::new();
    scalar::register(&mut worker);
    worker.run();
}
