//! The single-field accessor scalars over a `ua VARCHAR` argument:
//!
//! - `ua_browser`, `ua_browser_version`
//! - `ua_os`, `ua_os_version`
//! - `ua_device`, `ua_device_brand`  → `VARCHAR`
//! - `ua_is_bot`                      → `BOOLEAN`
//!
//! Each is arity-1, positional. NULL input → NULL output. Unknown / `"Other"`
//! families map to NULL (see `useragent.rs`).

use std::sync::Arc;

use arrow_array::builder::{BooleanBuilder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::DataType;
use vgi::{
    ArgSpec, BindParams, BindResponse, FunctionExample, FunctionMetadata, ProcessParams,
    ScalarFunction,
};
use vgi_rpc::{Result, RpcError};

use crate::arrow_io::text_str;
use crate::useragent;

/// Which VARCHAR field a [`UaField`] scalar extracts.
#[derive(Clone, Copy)]
enum Field {
    Browser,
    BrowserVersion,
    Os,
    OsVersion,
    Device,
    DeviceBrand,
}

impl Field {
    fn name(self) -> &'static str {
        match self {
            Field::Browser => "ua_browser",
            Field::BrowserVersion => "ua_browser_version",
            Field::Os => "ua_os",
            Field::OsVersion => "ua_os_version",
            Field::Device => "ua_device",
            Field::DeviceBrand => "ua_device_brand",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Field::Browser => "Browser/client family from a User-Agent (e.g. 'Chrome'), or NULL",
            Field::BrowserVersion => "Browser version from a User-Agent (e.g. '120.0.0'), or NULL",
            Field::Os => {
                "Operating-system family from a User-Agent (e.g. 'Windows', 'iOS'), or NULL"
            }
            Field::OsVersion => "Operating-system version from a User-Agent (e.g. '17.0'), or NULL",
            Field::Device => "Device family/model from a User-Agent (e.g. 'iPhone'), or NULL",
            Field::DeviceBrand => "Device brand from a User-Agent (e.g. 'Apple'), or NULL",
        }
    }

    /// A worked example query for this field's scalar, used by `vgi-lint`.
    fn example(self) -> FunctionExample {
        // A common desktop Chrome-on-Windows User-Agent, reused across fields.
        const CHROME_WIN: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
        let (fn_name, description) = match self {
            Field::Browser => (
                "ua_browser",
                "Extract the browser family ('Chrome') from a User-Agent.",
            ),
            Field::BrowserVersion => (
                "ua_browser_version",
                "Extract the browser version ('120.0.0') from a User-Agent.",
            ),
            Field::Os => (
                "ua_os",
                "Extract the operating-system family ('Windows') from a User-Agent.",
            ),
            Field::OsVersion => (
                "ua_os_version",
                "Extract the operating-system version ('10') from a User-Agent.",
            ),
            Field::Device => (
                "ua_device",
                "Extract the device family from a User-Agent (NULL for a generic desktop).",
            ),
            Field::DeviceBrand => (
                "ua_device_brand",
                "Extract the device brand from a User-Agent (NULL for a generic desktop).",
            ),
        };
        FunctionExample {
            sql: format!("SELECT useragent.main.{fn_name}('{CHROME_WIN}');"),
            description: description.to_string(),
            expected_output: None,
        }
    }

    /// The five standard per-object discovery/description tags for this field's
    /// scalar (VGI112/113/124/126/128). The title carries an extra word beyond
    /// the machine name so it never normalize-equals it (VGI125).
    fn object_tags(self) -> Vec<(String, String)> {
        let (title, doc_llm, doc_md, keywords) = match self {
            Field::Browser => (
                "Browser Family Name",
                "## ua_browser\n\nExtracts the **browser / client family** from an HTTP \
                 `User-Agent` header string and returns it as a `VARCHAR`.\n\n**When to use:** \
                 segment web-analytics traffic by browser, build top-browser reports, or filter \
                 rows to a specific client. Pair with `ua_browser_version` for the full client \
                 identity.\n\n**Input:** one `ua VARCHAR` column or literal (positional, \
                 arity-1).\n**Output:** the family name such as `Chrome`, `Safari`, `Firefox`, \
                 `Edge`, or `Mobile Safari`.\n\n**Edge cases:** `NULL`, empty, or unidentifiable \
                 input returns `NULL` (never the literal `'Other'`). Identification comes from \
                 the embedded uap-core regex database, so very new or obscure clients may not \
                 match.",
                "# Browser family\n\n`ua_browser(ua)` returns the browser/client family parsed \
                 from a User-Agent string.\n\n## Usage\n\n```sql\nSELECT ua_browser(ua) AS \
                 browser FROM hits;\n-- 'Chrome', 'Safari', 'Firefox', …\n```\n\n## Notes\n\n- \
                 NULL/empty/unknown input yields NULL.\n- Use `ua_browser_version` for the \
                 version component.",
                "browser, client, browser family, chrome, safari, firefox, edge, ua_browser, \
                 user-agent",
            ),
            Field::BrowserVersion => (
                "Browser Version String",
                "## ua_browser_version\n\nExtracts the **browser / client version** from an HTTP \
                 `User-Agent` header and returns it as a dotted `VARCHAR` (for example \
                 `120.0.0`).\n\n**When to use:** track adoption of browser releases, gate \
                 features by minimum version, or detect outdated clients in web-analytics \
                 data.\n\n**Input:** one `ua VARCHAR` column or literal.\n**Output:** a \
                 `major.minor.patch` string assembled from the components uap-core reports; only \
                 the leading run of present components is kept.\n\n**Edge cases:** `NULL`, empty, \
                 unparseable, or version-less input returns `NULL`. Version numbers track the \
                 embedded uap-core database and can drift across releases, so prefer ranges over \
                 exact-equality comparisons.",
                "# Browser version\n\n`ua_browser_version(ua)` returns the dotted browser version \
                 parsed from a User-Agent string.\n\n## Usage\n\n```sql\nSELECT \
                 ua_browser_version(ua) AS ver FROM hits;\n-- '120.0.0'\n```\n\n## Notes\n\n- \
                 NULL/empty/version-less input yields NULL.\n- Versions may drift with uap-core \
                 updates; compare with ranges, not exact equality.",
                "browser version, client version, version number, ua_browser_version, user-agent",
            ),
            Field::Os => (
                "Operating System Name",
                "## ua_os\n\nExtracts the **operating-system family** from an HTTP `User-Agent` \
                 header and returns it as a `VARCHAR`.\n\n**When to use:** break web traffic down \
                 by platform (Windows vs. iOS vs. Android), build OS-mix dashboards, or filter \
                 rows by platform. Pair with `ua_os_version` for the full OS identity.\n\n\
                 **Input:** one `ua VARCHAR` column or literal.\n**Output:** a family name such \
                 as `Windows`, `iOS`, `Android`, `Mac OS X`, or `Linux`.\n\n**Edge cases:** \
                 `NULL`, empty, or unidentifiable input returns `NULL` (never `'Other'`).",
                "# Operating system\n\n`ua_os(ua)` returns the operating-system family parsed \
                 from a User-Agent string.\n\n## Usage\n\n```sql\nSELECT ua_os(ua) AS os FROM \
                 hits;\n-- 'Windows', 'iOS', 'Android', …\n```\n\n## Notes\n\n- \
                 NULL/empty/unknown input yields NULL.\n- Use `ua_os_version` for the version \
                 component.",
                "os, operating system, platform, windows, ios, android, macos, linux, ua_os, \
                 user-agent",
            ),
            Field::OsVersion => (
                "Operating System Version",
                "## ua_os_version\n\nExtracts the **operating-system version** from an HTTP \
                 `User-Agent` header and returns it as a dotted `VARCHAR` (for example `17.0` or \
                 `10`).\n\n**When to use:** measure OS upgrade adoption, detect end-of-life \
                 platforms, or correlate behavior with platform version in web analytics.\n\n\
                 **Input:** one `ua VARCHAR` column or literal.\n**Output:** a dotted version \
                 string assembled from the components uap-core reports.\n\n**Edge cases:** \
                 `NULL`, empty, unparseable, or version-less input returns `NULL`. Values track \
                 the embedded uap-core database; prefer ranges over exact comparisons.",
                "# Operating-system version\n\n`ua_os_version(ua)` returns the dotted OS version \
                 parsed from a User-Agent string.\n\n## Usage\n\n```sql\nSELECT ua_os_version(ua) \
                 AS os_ver FROM hits;\n-- '17.0', '10'\n```\n\n## Notes\n\n- \
                 NULL/empty/version-less input yields NULL.\n- Versions may drift with uap-core \
                 updates.",
                "os version, operating system version, platform version, ua_os_version, \
                 user-agent",
            ),
            Field::Device => (
                "Device Family Model",
                "## ua_device\n\nExtracts the **device family / model** from an HTTP `User-Agent` \
                 header and returns it as a `VARCHAR` (for example `iPhone` or `Pixel 7`).\n\n\
                 **When to use:** segment traffic by hardware, build device-popularity reports, \
                 or distinguish mobile from desktop. Pair with `ua_device_brand` for the \
                 manufacturer.\n\n**Input:** one `ua VARCHAR` column or literal.\n**Output:** a \
                 device family/model string.\n\n**Edge cases:** generic desktop browsers, bots, \
                 and unidentifiable input return `NULL` — desktop User-Agents typically carry no \
                 device signal. For bots, the device is deliberately suppressed to `NULL` (use \
                 `ua_is_bot` instead).",
                "# Device family\n\n`ua_device(ua)` returns the device family/model parsed from a \
                 User-Agent string.\n\n## Usage\n\n```sql\nSELECT ua_device(ua) AS device FROM \
                 hits;\n-- 'iPhone', 'Pixel 7'\n```\n\n## Notes\n\n- NULL for generic desktops, \
                 bots, and unknown devices.\n- Use `ua_device_brand` for the manufacturer.",
                "device, device family, model, phone, tablet, iphone, pixel, ua_device, \
                 user-agent",
            ),
            Field::DeviceBrand => (
                "Device Brand Maker",
                "## ua_device_brand\n\nExtracts the **device brand / manufacturer** from an HTTP \
                 `User-Agent` header and returns it as a `VARCHAR` (for example `Apple`, \
                 `Samsung`, or `Google`).\n\n**When to use:** group traffic by vendor, build \
                 manufacturer market-share reports, or join against a hardware catalog. Pair \
                 with `ua_device` for the specific model.\n\n**Input:** one `ua VARCHAR` column \
                 or literal.\n**Output:** the brand/manufacturer name.\n\n**Edge cases:** \
                 generic desktop browsers, bots, and unidentifiable input return `NULL`; brand \
                 is suppressed to `NULL` for bots.",
                "# Device brand\n\n`ua_device_brand(ua)` returns the device brand/manufacturer \
                 parsed from a User-Agent string.\n\n## Usage\n\n```sql\nSELECT \
                 ua_device_brand(ua) AS brand FROM hits;\n-- 'Apple', 'Samsung', 'Google'\n```\n\
                 \n## Notes\n\n- NULL for generic desktops, bots, and unknown brands.\n- Use \
                 `ua_device` for the specific model.",
                "device brand, manufacturer, maker, apple, samsung, google, ua_device_brand, \
                 user-agent",
            ),
        };
        crate::meta::object_tags(title, doc_llm, doc_md, keywords, "scalar/fields.rs")
    }

    fn extract(self, ua: &str) -> Option<String> {
        match self {
            Field::Browser => useragent::browser(ua),
            Field::BrowserVersion => useragent::browser_version(ua),
            Field::Os => useragent::os(ua),
            Field::OsVersion => useragent::os_version(ua),
            Field::Device => useragent::device(ua),
            Field::DeviceBrand => useragent::device_brand(ua),
        }
    }
}

/// A VARCHAR-returning, arity-1 scalar that pulls one field out of a parsed UA.
pub struct UaField(Field);

impl UaField {
    pub fn browser() -> Self {
        Self(Field::Browser)
    }
    pub fn browser_version() -> Self {
        Self(Field::BrowserVersion)
    }
    pub fn os() -> Self {
        Self(Field::Os)
    }
    pub fn os_version() -> Self {
        Self(Field::OsVersion)
    }
    pub fn device() -> Self {
        Self(Field::Device)
    }
    pub fn device_brand() -> Self {
        Self(Field::DeviceBrand)
    }
}

impl ScalarFunction for UaField {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn metadata(&self) -> FunctionMetadata {
        FunctionMetadata {
            description: self.0.description().into(),
            return_type: Some(DataType::Utf8),
            examples: vec![self.0.example()],
            tags: self.0.object_tags(),
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::any_column(
            "ua",
            0,
            "The HTTP User-Agent header value to parse; the requested field (browser, OS, or \
             device component) is extracted from it.",
        )]
    }

    fn on_bind(&self, _params: &BindParams) -> Result<BindResponse> {
        Ok(BindResponse::result(DataType::Utf8))
    }

    fn process(&self, params: &ProcessParams, batch: &RecordBatch) -> Result<RecordBatch> {
        let col = batch.column(0);
        let rows = batch.num_rows();
        let mut out = StringBuilder::new();
        for i in 0..rows {
            match text_str(col, i)? {
                None => out.append_null(),
                Some(ua) => match self.0.extract(ua) {
                    Some(v) => out.append_value(v),
                    None => out.append_null(),
                },
            }
        }
        let arr: ArrayRef = Arc::new(out.finish());
        RecordBatch::try_new(params.output_schema.clone(), vec![arr])
            .map_err(|e| RpcError::runtime_error(e.to_string()))
    }
}

/// `ua_is_bot(ua) -> BOOLEAN`: true for spiders/crawlers (uap device family
/// "Spider"). NULL in → NULL out.
pub struct UaIsBot;

impl ScalarFunction for UaIsBot {
    fn name(&self) -> &str {
        "ua_is_bot"
    }

    fn metadata(&self) -> FunctionMetadata {
        FunctionMetadata {
            description:
                "True if the User-Agent is a spider/crawler (e.g. Googlebot), else false; \
                          NULL in → NULL out"
                    .into(),
            return_type: Some(DataType::Boolean),
            examples: vec![FunctionExample {
                sql: "SELECT useragent.main.ua_is_bot('Mozilla/5.0 (compatible; Googlebot/2.1; \
                      +http://www.google.com/bot.html)');"
                    .into(),
                description:
                    "Detect that the Googlebot crawler User-Agent is a bot (returns true).".into(),
                expected_output: None,
            }],
            tags: crate::meta::object_tags(
                "Detect Bot Crawler",
                "## ua_is_bot\n\nReturns a `BOOLEAN` that is `TRUE` when an HTTP `User-Agent` \
                 string identifies a **spider / crawler / bot** (for example Googlebot, Bingbot, \
                 or other automated agents) and `FALSE` for ordinary human browsers.\n\n**When \
                 to use:** filter automated traffic out of web-analytics aggregates, route bot \
                 requests differently, or compute a human-vs-bot split. A typical pattern is \
                 `WHERE NOT ua_is_bot(ua)` to keep only real visitors.\n\n**Input:** one `ua \
                 VARCHAR` column or literal.\n**Output:** `TRUE`/`FALSE`, or `NULL` when the \
                 input is `NULL`.\n\n**How it works:** detection is driven by the embedded \
                 uap-core regex database, which classifies crawlers under the device family \
                 `Spider`. Unrecognized agents are treated as non-bots (`FALSE`).",
                "# Bot detection\n\n`ua_is_bot(ua)` returns TRUE for spiders/crawlers and FALSE \
                 for ordinary browsers.\n\n## Usage\n\n```sql\n-- Keep only human traffic\nSELECT \
                 * FROM hits WHERE NOT ua_is_bot(ua);\n```\n\n## Notes\n\n- NULL input yields \
                 NULL.\n- Backed by uap-core's `Spider` device family.",
                "bot, crawler, spider, googlebot, bingbot, robot, automated traffic, filter, \
                 ua_is_bot, user-agent",
                "scalar/fields.rs",
            ),
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::any_column(
            "ua",
            0,
            "The HTTP User-Agent header value to test for being a spider/crawler (bot).",
        )]
    }

    fn on_bind(&self, _params: &BindParams) -> Result<BindResponse> {
        Ok(BindResponse::result(DataType::Boolean))
    }

    fn process(&self, params: &ProcessParams, batch: &RecordBatch) -> Result<RecordBatch> {
        let col = batch.column(0);
        let rows = batch.num_rows();
        let mut out = BooleanBuilder::new();
        for i in 0..rows {
            match text_str(col, i)? {
                None => out.append_null(),
                Some(ua) => out.append_value(useragent::is_bot(ua)),
            }
        }
        let arr: ArrayRef = Arc::new(out.finish());
        RecordBatch::try_new(params.output_schema.clone(), vec![arr])
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

    fn str_at(arr: &ArrayRef, i: usize) -> Option<String> {
        if arr.is_null(i) {
            None
        } else {
            Some(arr.as_string::<i32>().value(i).to_string())
        }
    }

    #[test]
    fn browser_and_os_bind_utf8_and_extract() {
        let f = UaField::browser();
        assert_eq!(bound_type(&f), DataType::Utf8);
        let out = run_scalar_text(&f, &[Some(CHROME_WIN), None], Arguments::default()).unwrap();
        assert_eq!(str_at(&out, 0).as_deref(), Some("Chrome"));
        assert!(out.is_null(1), "NULL in → NULL out");

        let osf = UaField::os();
        let out = run_scalar_text(
            &osf,
            &[Some(CHROME_WIN), Some(IPHONE)],
            Arguments::default(),
        )
        .unwrap();
        assert_eq!(str_at(&out, 0).as_deref(), Some("Windows"));
        assert_eq!(str_at(&out, 1).as_deref(), Some("iOS"));
    }

    #[test]
    fn device_and_brand() {
        let out =
            run_scalar_text(&UaField::device(), &[Some(IPHONE)], Arguments::default()).unwrap();
        assert_eq!(str_at(&out, 0).as_deref(), Some("iPhone"));
        let out = run_scalar_text(
            &UaField::device_brand(),
            &[Some(IPHONE)],
            Arguments::default(),
        )
        .unwrap();
        assert_eq!(str_at(&out, 0).as_deref(), Some("Apple"));
    }

    #[test]
    fn garbage_yields_null_field() {
        let out = run_scalar_text(
            &UaField::browser(),
            &[Some("garbage"), Some("")],
            Arguments::default(),
        )
        .unwrap();
        assert!(out.is_null(0));
        assert!(out.is_null(1));
    }

    #[test]
    fn is_bot_binds_boolean_and_detects_googlebot() {
        assert_eq!(bound_type(&UaIsBot), DataType::Boolean);
        let out = run_scalar_text(
            &UaIsBot,
            &[Some(GOOGLEBOT), Some(CHROME_WIN), None],
            Arguments::default(),
        )
        .unwrap();
        let b = out.as_boolean();
        assert!(b.value(0), "Googlebot must be a bot");
        assert!(!b.value(1), "Chrome must not be a bot");
        assert!(out.is_null(2), "NULL in → NULL out");
    }
}
