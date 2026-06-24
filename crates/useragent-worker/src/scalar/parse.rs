//! `ua_parse(ua)` → `STRUCT(browser VARCHAR, browser_version VARCHAR,
//! os VARCHAR, os_version VARCHAR, device VARCHAR, brand VARCHAR,
//! is_bot BOOLEAN)`. A one-shot parse: every field computed from a single pass
//! over the UA string. NULL / empty / unparseable input → a NULL struct row.

use std::sync::Arc;

use arrow_array::builder::{BooleanBuilder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch, StructArray};
use arrow_buffer::NullBuffer;
use vgi::{
    ArgSpec, BindParams, BindResponse, FunctionExample, FunctionMetadata, ProcessParams,
    ScalarFunction,
};
use vgi_rpc::{Result, RpcError};

use crate::arrow_io::{parse_struct_fields, parse_struct_type, text_str};
use crate::useragent;

/// Guaranteed-runnable, catalog-qualified examples (VGI509). Each `sql` is
/// self-contained and re-runnable against an attached `useragent` worker. We
/// omit `expected_result` deliberately — the linter only needs each query to
/// execute cleanly, and uap-core version numbers drift across releases.
const EXECUTABLE_EXAMPLES: &str = r#"[
  {
    "description": "Extract the browser family from a desktop Chrome User-Agent.",
    "sql": "SELECT useragent.main.ua_browser('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36') AS browser"
  },
  {
    "description": "Extract the operating system from an iPhone Safari User-Agent.",
    "sql": "SELECT useragent.main.ua_os('Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1') AS os"
  },
  {
    "description": "Detect that the Googlebot crawler is a bot.",
    "sql": "SELECT useragent.main.ua_is_bot('Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)') AS is_bot"
  },
  {
    "description": "Parse every field of a Chrome-on-Windows User-Agent at once.",
    "sql": "SELECT useragent.main.ua_parse('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36') AS parsed"
  },
  {
    "description": "Report the running worker version string.",
    "sql": "SELECT useragent.main.useragent_version() AS version"
  }
]"#;

pub struct UaParse;

impl ScalarFunction for UaParse {
    fn name(&self) -> &str {
        "ua_parse"
    }

    fn metadata(&self) -> FunctionMetadata {
        let mut tags = crate::meta::object_tags(
            "Parse User-Agent Fields",
            "## ua_parse\n\nParses an HTTP `User-Agent` string into a **single `STRUCT`** \
             containing every component at once, in one pass over the input.\n\n**Returned \
             struct fields:**\n\n- `browser` (VARCHAR) — client family\n- `browser_version` \
             (VARCHAR) — dotted client version\n- `os` (VARCHAR) — operating-system family\n- \
             `os_version` (VARCHAR) — dotted OS version\n- `device` (VARCHAR) — device \
             family/model\n- `brand` (VARCHAR) — device brand/manufacturer\n- `is_bot` (BOOLEAN) \
             — spider/crawler flag\n\n**When to use:** prefer `ua_parse` over calling the \
             individual `ua_*` accessors when you need several fields, since it parses the string \
             only once. Project with `(ua_parse(ua)).*` or pick fields like \
             `(ua_parse(ua)).os`.\n\n**Edge cases:** `NULL`, empty, or unparseable input yields \
             a `NULL` struct row. For bots, `device` and `brand` are suppressed to `NULL` while \
             `is_bot` is `TRUE`. Unidentified individual fields are `NULL` rather than `'Other'`.",
            "# Parse all fields\n\n`ua_parse(ua)` returns a STRUCT with every parsed User-Agent \
             field in one pass.\n\n## Usage\n\n```sql\n-- Explode all fields\nSELECT \
             (ua_parse(ua)).* FROM hits;\n\n-- Pick a couple\nSELECT (ua_parse(ua)).browser, \
             (ua_parse(ua)).os FROM hits;\n```\n\n## Struct shape\n\n`STRUCT(browser, \
             browser_version, os, os_version, device, brand, is_bot)`\n\n## Notes\n\n- More \
             efficient than calling each `ua_*` accessor separately.\n- NULL/unparseable input → \
             NULL row.",
            "ua_parse, parse user-agent, user agent struct, browser, os, device, brand, is_bot, \
             one-shot parse, struct",
            "scalar/parse.rs",
        );
        tags.push(("vgi.executable_examples".into(), EXECUTABLE_EXAMPLES.into()));
        FunctionMetadata {
            description: "Parse a User-Agent into STRUCT(browser, browser_version, os, \
                          os_version, device, brand, is_bot); NULL/unparseable → NULL row"
                .into(),
            examples: vec![FunctionExample {
                sql: "SELECT useragent.main.ua_parse('Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 \
                      like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 \
                      Mobile/15E148 Safari/604.1') AS parsed;"
                    .into(),
                description: "Parse an iPhone Safari User-Agent into all of its fields at once \
                              (browser, OS, device, brand, is_bot)."
                    .into(),
                expected_output: None,
            }],
            tags,
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::any_column("ua", 0, "User-Agent string (VARCHAR)")]
    }

    fn on_bind(&self, _params: &BindParams) -> Result<BindResponse> {
        Ok(BindResponse::result(parse_struct_type()))
    }

    fn process(&self, params: &ProcessParams, batch: &RecordBatch) -> Result<RecordBatch> {
        let col = batch.column(0);
        let rows = batch.num_rows();

        let mut browser = StringBuilder::new();
        let mut browser_version = StringBuilder::new();
        let mut os = StringBuilder::new();
        let mut os_version = StringBuilder::new();
        let mut device = StringBuilder::new();
        let mut brand = StringBuilder::new();
        let mut is_bot = BooleanBuilder::new();
        let mut valid: Vec<bool> = Vec::with_capacity(rows);

        let append_opt = |b: &mut StringBuilder, v: &Option<String>| match v {
            Some(s) => b.append_value(s),
            None => b.append_null(),
        };

        for i in 0..rows {
            match text_str(col, i)? {
                None => {
                    browser.append_null();
                    browser_version.append_null();
                    os.append_null();
                    os_version.append_null();
                    device.append_null();
                    brand.append_null();
                    is_bot.append_null();
                    valid.push(false);
                }
                Some(ua) => {
                    let p = useragent::parse(ua);
                    append_opt(&mut browser, &p.browser);
                    append_opt(&mut browser_version, &p.browser_version);
                    append_opt(&mut os, &p.os);
                    append_opt(&mut os_version, &p.os_version);
                    append_opt(&mut device, &p.device);
                    append_opt(&mut brand, &p.brand);
                    is_bot.append_value(p.is_bot);
                    valid.push(true);
                }
            }
        }

        let arrays: Vec<ArrayRef> = vec![
            Arc::new(browser.finish()),
            Arc::new(browser_version.finish()),
            Arc::new(os.finish()),
            Arc::new(os_version.finish()),
            Arc::new(device.finish()),
            Arc::new(brand.finish()),
            Arc::new(is_bot.finish()),
        ];
        let out: ArrayRef = Arc::new(StructArray::new(
            parse_struct_fields(),
            arrays,
            Some(NullBuffer::from(valid)),
        ));
        RecordBatch::try_new(params.output_schema.clone(), vec![out])
            .map_err(|e| RpcError::runtime_error(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arrow_io::test_support::{bound_type, run_scalar_text};
    use arrow_array::cast::AsArray;
    use arrow_array::Array;
    use vgi::arguments::Arguments;

    const CHROME_WIN: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
        (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
    const IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
        AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
    const GOOGLEBOT: &str =
        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";

    fn col_str(s: &StructArray, idx: usize, row: usize) -> Option<String> {
        let c = s.column(idx);
        if c.is_null(row) {
            None
        } else {
            Some(c.as_string::<i32>().value(row).to_string())
        }
    }

    #[test]
    fn bind_declares_the_struct_the_process_builds() {
        assert_eq!(bound_type(&UaParse), parse_struct_type());
    }

    #[test]
    fn process_parses_struct_fields() {
        let out = run_scalar_text(
            &UaParse,
            &[
                Some(CHROME_WIN),
                Some(IPHONE),
                Some(GOOGLEBOT),
                None,
                Some("garbage"),
            ],
            Arguments::default(),
        )
        .unwrap();
        assert_eq!(out.data_type(), &parse_struct_type());
        let s = out.as_struct();

        // Chrome / Windows.
        assert_eq!(col_str(s, 0, 0).as_deref(), Some("Chrome")); // browser
        assert_eq!(col_str(s, 2, 0).as_deref(), Some("Windows")); // os
        assert!(!s.column(6).as_boolean().value(0)); // is_bot

        // iPhone / iOS.
        assert_eq!(col_str(s, 2, 1).as_deref(), Some("iOS")); // os
        assert_eq!(col_str(s, 4, 1).as_deref(), Some("iPhone")); // device
        assert_eq!(col_str(s, 5, 1).as_deref(), Some("Apple")); // brand

        // Googlebot is a bot.
        assert!(s.column(6).as_boolean().value(2));

        // NULL input → NULL struct row.
        assert!(s.is_null(3));

        // garbage → non-null row but all-NULL fields, is_bot false.
        assert!(!s.is_null(4));
        assert!(col_str(s, 0, 4).is_none());
        assert!(col_str(s, 2, 4).is_none());
        assert!(!s.column(6).as_boolean().value(4));
    }
}
