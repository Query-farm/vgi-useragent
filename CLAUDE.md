# CLAUDE.md — vgi-useragent

Contributor/agent notes. User-facing docs live in `README.md`; this is the
"how it's built and where the sharp edges are" companion.

## What this is

A [VGI](https://query.farm) worker (Rust, compiled binary) exposing **HTTP
User-Agent parsing** (browser / OS / device + bot detection) to DuckDB/SQL over
Arrow IPC. Built on the `vgi` crate (crates.io), modeled on `vgi-image` /
`vgi-ioc` / `vgi-barcode`. Catalog name `useragent` (single `main` schema). Pure
text processing: the only non-trivial dep is `uaparser` (+ `once_cell`).

## Layout

```
Cargo.toml                          workspace; pins vgi = "0.5.0", uaparser, once_cell
crates/useragent-worker/
  data/regexes.yaml                 uap-core regex DB (Apache-2.0), embedded via include_bytes!
  data/UAP-CORE-LICENSE             uap-core's Apache-2.0 license
  src/main.rs                       Worker::new(); registers scalars
  src/useragent.rs                  PURE logic (no Arrow): parser wrapper + field normalization + unit tests
  src/arrow_io.rs                   VARCHAR reads + ua_parse STRUCT type + in-process scalar test harness
  src/scalar/{version,fields,parse,mod}.rs   thin Arrow scalar adapters
  tests/parse.rs                    integration tests (include useragent.rs by #[path], like vgi-ioc)
test/sql/*.test                     haybarn-unittest sqllogictest — authoritative E2E
Makefile                            test / test-unit / test-sql / lint / fmt / build / clean
```

Pattern: keep computation in `useragent.rs` (pure, unit-tested), keep Arrow
marshalling in `arrow_io.rs` + `scalar/*.rs` (thin, harness-tested).

## UA library & embedded data (the core design choice)

- Parser: the **`uaparser`** crate (0.6.4) — the uap-rust port of ua-parser. It
  builds cleanly on MSRV 1.86 (verified) and is pure regex (no native deps).
- Data: uap-core's `regexes.yaml` (~196 KiB), vendored under `data/` and
  **embedded with `include_bytes!`** so the binary is self-contained — there is
  no runtime file path. `UserAgentParser::from_bytes(REGEXES_YAML)` builds it.
- The parser is immutable and compiled **once per process** via
  `once_cell::sync::Lazy` (`PARSER`), then shared for the worker's lifetime.
- Licensing: `uaparser` is MIT; uap-core (`regexes.yaml`) is **Apache-2.0** (its
  license is vendored at `data/UAP-CORE-LICENSE`). The worker's own code is MIT.

## Field model / "Other" → NULL

uap-core returns the sentinel family `"Other"` (versions `None`) for anything it
can't identify. `useragent.rs` maps `"Other"` and empty strings to `None`, so
empty/garbage/NULL input surfaces as SQL **NULL**, not the literal `'Other'`.
Versions are assembled as dotted strings from major/minor/patch(/patch_minor),
keeping only the leading run of present components.

## Bot detection

uap-core classifies spiders/crawlers with **device family `"Spider"`**.
`is_bot` is `device.family == "Spider"`. For bots we suppress `device`/`brand`
to NULL (so those accessors stay about real hardware); the spider signal lives
in `ua_is_bot` / the `is_bot` struct field.

## Function surface

Scalars (all arity-1 `ua VARCHAR`, positional; NULL in → NULL out):
`ua_browser`, `ua_browser_version`, `ua_os`, `ua_os_version`, `ua_device`,
`ua_device_brand` → VARCHAR; `ua_is_bot` → BOOLEAN; `ua_parse` → STRUCT(browser,
browser_version, os, os_version, device, brand, is_bot). Plus `useragent_version()`.

## Sharp edges (learned from the templates)

1. **`haybarn-unittest` skips `require vgi`** — `.test` files use explicit
   `statement ok` + `LOAD vgi;`. Functions live under the `useragent` catalog, so
   each file does `SET search_path = 'useragent.main'`, then `USE memory` before
   `DETACH`.
2. **Scalars are positional-only.** All ours are arity-1 (or 0 for
   `useragent_version`); no optional args / overloads needed.
3. **STRUCT returns** need the Arrow `DataType` to match between `on_bind` and
   `process`. Centralized in `arrow_io::parse_struct_fields()` /
   `parse_struct_type()`; a mismatch makes DuckDB reject the batch. A NULL input
   row is a NULL `StructArray` entry (via the validity `NullBuffer`).
4. **NULL semantics:** scalar NULL in → NULL out; unknown field → NULL (not
   `'Other'`); `ua_parse(NULL)` → NULL struct row.
5. **Determinism in SQL tests:** the `ua_parse` VALUES query uses
   `ORDER BY ... NULLS FIRST` for stable comparison; assertions use stable
   *families* (e.g. `Chrome`, `iOS`), never exact version numbers (which drift
   with uap-core updates).
6. **Bounded input / never panics:** input is truncated to 64 KiB at a char
   boundary before matching (UA headers are tiny; this just caps pathological
   input).

## Tests

- `cargo test --workspace` — `useragent.rs` unit tests (known UAs: Chrome/Win,
  iPhone/iOS, Android/Chrome, Googlebot→bot, empty/garbage→NULL, version
  assembly, bounding) + the in-process Arrow-boundary tests in each `scalar/*.rs`
  + `tests/parse.rs`.
- `make test-sql` — the DuckDB E2E suite in `test/sql/*.test`.
