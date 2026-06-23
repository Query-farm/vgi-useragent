<p align="center">
  <img src="https://raw.githubusercontent.com/Query-farm/vgi/main/docs/vgi-logo.png" alt="Vector Gateway Interface (VGI)" width="320">
</p>

<p align="center"><em>A <a href="https://query.farm">Query.Farm</a> VGI worker for DuckDB.</em></p>

# Parse User-Agent Strings to Browser, OS & Device in DuckDB

> **vgi-useragent** · a [Query.Farm](https://query.farm) VGI worker · powered by uap-core

A [VGI](https://query.farm) worker (Rust, a compiled binary) that brings
**HTTP User-Agent parsing** to DuckDB / SQL over Apache Arrow. DuckDB launches
the worker and talks to it over Arrow IPC; the functions appear under the
catalog `useragent`, schema `main`.

It parses a User-Agent string into its **browser**, **operating system**, and
**device** components (plus spider/crawler detection), using the
[ua-parser](https://github.com/ua-parser) regex database. Pure text processing —
no network access, no native dependencies, and the regex data is embedded in the
binary so the worker is fully self-contained.

```sql
LOAD vgi;
ATTACH 'useragent' (TYPE vgi, LOCATION './target/release/useragent-worker');
SET search_path = 'useragent.main';

-- Individual fields.
SELECT ua_browser('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36');
-- → 'Chrome'
SELECT ua_os('Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1');
-- → 'iOS'
SELECT ua_device('Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) ...');
-- → 'iPhone'

-- Spider / crawler detection.
SELECT ua_is_bot('Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)');
-- → true

-- One-shot parse into a STRUCT.
SELECT (ua_parse(ua)).*  FROM hits;
-- browser | browser_version | os | os_version | device | brand | is_bot
```

## Functions

All scalars take a single `ua VARCHAR` argument (positional). **NULL input →
NULL output.** Unknown / unparseable fields (ua-parser's `"Other"` sentinel) are
mapped to **NULL** rather than the literal string `'Other'`.

| Function | Returns | Description |
| --- | --- | --- |
| `ua_browser(ua)` | `VARCHAR` | Browser/client family, e.g. `'Chrome'`. |
| `ua_browser_version(ua)` | `VARCHAR` | Browser version, e.g. `'120.0.0'`. |
| `ua_os(ua)` | `VARCHAR` | OS family, e.g. `'Windows'`, `'iOS'`. |
| `ua_os_version(ua)` | `VARCHAR` | OS version, e.g. `'17.0'`. |
| `ua_device(ua)` | `VARCHAR` | Device family/model, e.g. `'iPhone'`. |
| `ua_device_brand(ua)` | `VARCHAR` | Device brand, e.g. `'Apple'`. |
| `ua_is_bot(ua)` | `BOOLEAN` | `true` for spiders/crawlers (e.g. Googlebot). |
| `ua_parse(ua)` | `STRUCT(...)` | One-shot parse (all fields below). |
| `useragent_version()` | `VARCHAR` | Worker version string. |

`ua_parse` returns
`STRUCT(browser VARCHAR, browser_version VARCHAR, os VARCHAR, os_version VARCHAR,
device VARCHAR, brand VARCHAR, is_bot BOOLEAN)`. A NULL input yields a NULL
struct row; an unparseable input yields a non-null row with NULL string fields
and `is_bot = false`.

For a bot (spider), `ua_device` / `ua_device_brand` (and the `device` / `brand`
struct fields) are NULL — the spider signal is carried by `ua_is_bot` /
`is_bot`.

## Data source & licensing

- **Parsing engine:** the [`uaparser`](https://crates.io/crates/uaparser) crate
  (the uap-rust implementation of ua-parser). MIT-licensed.
- **Regex database:** the upstream
  [uap-core](https://github.com/ua-parser/uap-core) `regexes.yaml`, vendored at
  `crates/useragent-worker/data/regexes.yaml` and **embedded into the binary at
  compile time** via `include_bytes!`. The worker needs no external file at
  runtime. uap-core is **Apache-2.0**; its license is included alongside the
  data at `crates/useragent-worker/data/UAP-CORE-LICENSE`.
- This worker's own source is **MIT** (see `LICENSE`).

The parser is compiled exactly once per process (lazily, on first use) and
reused for the lifetime of the worker.

## Building & testing

```sh
cargo build --release            # produces target/release/useragent-worker
cargo test --workspace           # pure-logic + Arrow-boundary + integration tests
make lint                        # clippy (deny warnings) + rustfmt --check
make test-sql                    # DuckDB sqllogictest E2E (needs haybarn-unittest)
```

The SQL E2E suite drives the compiled worker through DuckDB via
`haybarn-unittest` (`uv tool install haybarn-unittest`; ensure
`~/.local/bin` is on `PATH`).

---

## Authorship & License

Written by [Query.Farm](https://query.farm).

Copyright 2026 Query Farm LLC - https://query.farm

