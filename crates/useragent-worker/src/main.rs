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
mod meta;
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
                "vgi.title".to_string(),
                "HTTP User-Agent Parsing & Bot Detection".to_string(),
            ),
            (
                "vgi.keywords".to_string(),
                meta::keywords_json(
                    "useragent, user-agent, user agent, UA, browser, operating system, OS, \
                     device, device brand, bot, crawler, spider, web analytics, traffic, parse, \
                     uap-core",
                ),
            ),
            (
                "vgi.doc_llm".to_string(),
                "## useragent worker\n\nParses HTTP `User-Agent` header strings into their \
                 **browser/client**, **operating-system**, and **device** components, and \
                 detects **spiders/crawlers (bots)** — all over Apache Arrow inside DuckDB.\n\n\
                 **What you can do:** extract a single field (browser, browser version, OS, OS \
                 version, device, device brand), test whether a User-Agent is a bot, or get every \
                 field at once as a `STRUCT` via `ua_parse`.\n\n**When to use:** web-analytics \
                 enrichment, traffic segmentation by client/platform/device, and bot filtering in \
                 SQL.\n\n**Behavior:** all scalars are arity-1 (`ua VARCHAR`); `NULL`/empty/\
                 unidentifiable input yields `NULL` (never the literal `'Other'`). Parsing is \
                 backed by the embedded uap-core regex database."
                    .to_string(),
            ),
            (
                "vgi.doc_md".to_string(),
                "# useragent — HTTP User-Agent Parsing & Bot Detection in SQL\n\n\
                 Turn raw HTTP `User-Agent` header strings into structured **browser**, \
                 **operating-system**, and **device** facts — and flag **bots, crawlers, and \
                 spiders** — directly in DuckDB SQL over Apache Arrow.\n\n\
                 The `useragent` extension brings production-grade User-Agent parsing to your SQL \
                 workflow. It is built for web-analytics teams, log-pipeline engineers, and anyone \
                 who needs to segment HTTP traffic by client, platform, or device without leaving \
                 the database. Point it at a column of raw `User-Agent` headers and get clean, \
                 normalized browser / OS / device attributes plus a reliable bot signal — no \
                 hand-written UDFs, no external services, and no row-by-row API round-trips.\n\n\
                 Parsing is powered by the community-maintained \
                 [uap-core](https://github.com/ua-parser/uap-core) regex database — the same \
                 ruleset behind the cross-language ua-parser project — executed through the Rust \
                 [`uaparser`](https://docs.rs/uaparser/latest/uaparser/) crate. The full uap-core \
                 `regexes.yaml` ruleset is embedded into the worker at compile time and compiled \
                 exactly once per process, so the binary is fully self-contained and matching \
                 stays fast across millions of rows. Results stream back to DuckDB over Apache \
                 Arrow. Unidentified families normalize to SQL `NULL` (never the literal \
                 `'Other'`), and crawlers are recognized via uap-core's `Spider` device \
                 classification.\n\n\
                 The function surface is small and composable. Use the single-field accessors \
                 `ua_browser`, `ua_browser_version`, `ua_os`, `ua_os_version`, `ua_device`, and \
                 `ua_device_brand` to pull one attribute at a time; call `ua_is_bot` for a \
                 `BOOLEAN` crawler/spider flag; or call `ua_parse` once to get every field at \
                 once as a `STRUCT(browser, browser_version, os, os_version, device, brand, \
                 is_bot)`. `useragent_version` reports the worker build. Every accessor takes a \
                 single `ua VARCHAR` argument and returns `NULL` for empty or unrecognizable \
                 input — perfect for `SELECT ua_browser(ua), ua_os(ua) FROM hits` style \
                 enrichment, traffic segmentation by platform/device, and bot filtering in \
                 SQL.\n\n\
                 ## Functions\n\n- `ua_browser`, `ua_browser_version`\n- `ua_os`, \
                 `ua_os_version`\n- `ua_device`, `ua_device_brand`\n- `ua_is_bot` (BOOLEAN)\n- \
                 `ua_parse` (STRUCT of all fields)\n- `useragent_version`\n\n\
                 ## Learn more\n\n\
                 - Source code: [ua-parser/uap-core](https://github.com/ua-parser/uap-core)\n\
                 - Specification: \
                 [uap-core spec](https://github.com/ua-parser/uap-core/blob/master/docs/specification.md)\n\
                 - Rust parser docs: \
                 [uaparser on docs.rs](https://docs.rs/uaparser/latest/uaparser/)"
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
                ("vgi.title".to_string(), "User-Agent — main".to_string()),
                (
                    "vgi.keywords".to_string(),
                    meta::keywords_json(
                        "useragent, user-agent, browser, os, operating system, device, \
                         device brand, bot, crawler, spider, ua_browser, ua_os, ua_device, \
                         ua_is_bot, ua_parse, web analytics",
                    ),
                ),
                // VGI123 classifying tags (bare keys: domain/category/topic) for faceting.
                ("domain".to_string(), "web-analytics".to_string()),
                ("category".to_string(), "parsing".to_string()),
                ("topic".to_string(), "user-agent-detection".to_string()),
                (
                    "vgi.doc_llm".to_string(),
                    "## useragent.main\n\nThe schema holding the User-Agent parsing and \
                     bot-detection functions. Extract the browser, OS, or device from a \
                     `User-Agent` string with the single-field `ua_*` accessors, detect \
                     spiders/crawlers with `ua_is_bot`, or parse every field at once into a \
                     `STRUCT` with `ua_parse`. Use these for web-analytics enrichment and traffic \
                     segmentation."
                        .to_string(),
                ),
                (
                    "vgi.doc_md".to_string(),
                    "# useragent.main\n\nUser-Agent parsing and bot-detection functions over \
                     Apache Arrow.\n\nIncludes the single-field accessors (`ua_browser`, `ua_os`, \
                     `ua_device`, …), the `ua_is_bot` predicate, and the one-shot `ua_parse` \
                     STRUCT function."
                        .to_string(),
                ),
                // VGI506 representative example queries for the schema.
                (
                    "vgi.example_queries".to_string(),
                    "SELECT useragent.main.ua_browser('Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                     AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36');\n\
                     SELECT useragent.main.ua_os('Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like \
                     Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 \
                     Mobile/15E148 Safari/604.1');\n\
                     SELECT useragent.main.ua_is_bot('Mozilla/5.0 (compatible; Googlebot/2.1; \
                     +http://www.google.com/bot.html)');\n\
                     SELECT useragent.main.ua_parse('Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                     AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36') \
                     AS parsed;"
                        .to_string(),
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
