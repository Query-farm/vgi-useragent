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

use vgi::catalog::{CatSchema, CatalogModel};
use vgi::Worker;

/// Worker version string, surfaced by `useragent_version()`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Catalog + schema metadata (description, provenance) surfaced to DuckDB and
/// the `vgi-lint` metadata-quality linter. The function objects themselves are
/// served from the registered scalars; this only adds catalog/schema-level
/// comments and tags.
fn catalog_metadata(name: &str) -> CatalogModel {
    CatalogModel {
        name: name.to_string(),
        comment: Some(
            "HTTP User-Agent parsing: browser / OS / device + bot detection.".to_string(),
        ),
        tags: vec![
            (
                "vgi.description_llm".to_string(),
                "Parse HTTP User-Agent header strings into their browser/client, operating \
                 system, and device components, and detect spiders/crawlers (bots). Extract a \
                 single field (browser, browser version, OS, OS version, device, device brand), \
                 test whether a User-Agent is a bot, or get every field at once as a STRUCT. \
                 Use for web-analytics enrichment, traffic segmentation, and bot filtering in SQL."
                    .to_string(),
            ),
            (
                "vgi.description_md".to_string(),
                "# useragent\n\nHTTP User-Agent parsing (browser / OS / device + bot detection) \
                 over Apache Arrow, backed by the uap-core regex database.\n\nScalars: \
                 `ua_browser`, `ua_browser_version`, `ua_os`, `ua_os_version`, `ua_device`, \
                 `ua_device_brand`, `ua_is_bot`, `ua_parse`, `useragent_version`."
                    .to_string(),
            ),
            ("vgi.author".to_string(), "Query.Farm".to_string()),
            (
                "vgi.copyright".to_string(),
                "Copyright 2026 Query Farm LLC - https://query.farm".to_string(),
            ),
            ("vgi.license".to_string(), "MIT".to_string()),
            (
                "vgi.support_contact".to_string(),
                "https://github.com/Query-farm/vgi-useragent/issues".to_string(),
            ),
            (
                "vgi.support_policy_url".to_string(),
                "https://github.com/Query-farm/vgi-useragent/blob/main/README.md".to_string(),
            ),
        ],
        source_url: Some("https://github.com/Query-farm/vgi-useragent".to_string()),
        schemas: vec![CatSchema {
            name: "main".to_string(),
            comment: Some("User-Agent parsing and bot-detection functions.".to_string()),
            tags: vec![
                (
                    "vgi.description_llm".to_string(),
                    "User-Agent parsing and bot-detection functions: extract the browser, OS, \
                     and device from a User-Agent string, detect spiders/crawlers, or parse all \
                     fields at once into a STRUCT."
                        .to_string(),
                ),
                (
                    "vgi.description_md".to_string(),
                    "User-Agent parsing and bot-detection functions over Apache Arrow.".to_string(),
                ),
            ],
            views: Vec::new(),
            macros: Vec::new(),
            tables: Vec::new(),
        }],
        ..Default::default()
    }
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
    let catalog_name =
        std::env::var("VGI_WORKER_CATALOG_NAME").unwrap_or_else(|_| "useragent".to_string());

    let mut worker = Worker::new();
    scalar::register(&mut worker);
    worker.set_catalog(catalog_metadata(&catalog_name));
    worker.run();
}
