//! Pure User-Agent parsing logic (no Arrow). Wraps the `uaparser` crate (the
//! uap-rust implementation of ua-parser, backed by the uap-core `regexes.yaml`)
//! and maps its results into the worker's field model.
//!
//! ## Data source / licensing
//!
//! Parsing is driven by `data/regexes.yaml` — the upstream **uap-core**
//! regex database (Apache-2.0; see `data/UAP-CORE-LICENSE`). It is embedded in
//! the binary with [`include_bytes!`], so the worker is fully self-contained and
//! needs **no external file at runtime**. The (immutable) parser is compiled
//! exactly once per process via [`once_cell::sync::Lazy`].
//!
//! ## "Other" → NULL
//!
//! uap-core returns the sentinel family `"Other"` (and `None` versions) when it
//! cannot identify a browser / OS / device. We treat `"Other"` and empty strings
//! as *unknown* and map them to `None`, so unparseable / empty / NULL input
//! surfaces as SQL NULL rather than the literal string `'Other'`.
//!
//! ## Bot detection
//!
//! uap-core classifies spiders/crawlers (Googlebot, bingbot, …) with the
//! **device family `"Spider"`**. [`is_bot`] reports `true` exactly when the
//! parsed device family is `"Spider"`.

use once_cell::sync::Lazy;
use uaparser::{Parser, UserAgentParser};

/// The uap-core regex database, embedded at compile time (Apache-2.0). Embedding
/// keeps the worker self-contained: no `regexes.yaml` on disk is consulted at
/// runtime.
static REGEXES_YAML: &[u8] = include_bytes!("../data/regexes.yaml");

/// Defensive upper bound on the input size handed to the regex engine. User-Agent
/// headers are tiny in practice (a few hundred bytes); anything pathologically
/// large is truncated at a UTF-8 char boundary so we never feed an unbounded
/// string to the matchers. Never panics.
const MAX_INPUT_BYTES: usize = 64 * 1024;

/// The process-wide parser, compiled once on first use from the embedded YAML.
static PARSER: Lazy<UserAgentParser> = Lazy::new(|| {
    UserAgentParser::from_bytes(REGEXES_YAML)
        .expect("embedded uap-core regexes.yaml must compile into a parser")
});

/// One-shot parse result. Every field is already normalized: `"Other"`/empty →
/// `None`, versions assembled as dotted strings, `is_bot` derived from the
/// device family.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    pub browser: Option<String>,
    pub browser_version: Option<String>,
    pub os: Option<String>,
    pub os_version: Option<String>,
    pub device: Option<String>,
    pub brand: Option<String>,
    pub is_bot: bool,
}

/// Truncate to [`MAX_INPUT_BYTES`] at a char boundary (no panic on multi-byte
/// splits).
fn bound(ua: &str) -> &str {
    if ua.len() <= MAX_INPUT_BYTES {
        return ua;
    }
    let mut end = MAX_INPUT_BYTES;
    while end > 0 && !ua.is_char_boundary(end) {
        end -= 1;
    }
    &ua[..end]
}

/// Normalize a uap family string: `"Other"` or empty → `None`.
fn norm_family(family: &str) -> Option<String> {
    match family.trim() {
        "" | "Other" => None,
        f => Some(f.to_string()),
    }
}

/// Normalize an optional uap replacement value: empty → `None`.
fn norm_opt(value: Option<&str>) -> Option<String> {
    match value {
        Some(v) if !v.trim().is_empty() => Some(v.to_string()),
        _ => None,
    }
}

/// Assemble a dotted version string from up to four components, dropping
/// trailing missing parts (`Some("17"), Some("0"), None` → `"17.0"`). Returns
/// `None` if no component is present.
fn version(parts: &[Option<&str>]) -> Option<String> {
    // Keep only the leading run of present components (so "17", None, "3" still
    // yields "17" rather than a misleading "17..3").
    let mut out: Vec<&str> = Vec::with_capacity(parts.len());
    for p in parts {
        match p {
            Some(v) if !v.trim().is_empty() => out.push(v.trim()),
            _ => break,
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out.join("."))
    }
}

/// Parse a User-Agent string into all fields at once.
pub fn parse(ua: &str) -> Parsed {
    let ua = bound(ua);
    let client = PARSER.parse(ua);

    let browser = norm_family(&client.user_agent.family);
    let browser_version = browser.as_ref().and_then(|_| {
        version(&[
            client.user_agent.major.as_deref(),
            client.user_agent.minor.as_deref(),
            client.user_agent.patch.as_deref(),
        ])
    });

    let os = norm_family(&client.os.family);
    let os_version = os.as_ref().and_then(|_| {
        version(&[
            client.os.major.as_deref(),
            client.os.minor.as_deref(),
            client.os.patch.as_deref(),
            client.os.patch_minor.as_deref(),
        ])
    });

    let is_bot = client.device.family == "Spider";

    // For spiders the device "family" is the literal "Spider"; surface NULL for
    // the device/brand of a bot so `ua_device`/`ua_device_brand` stay about real
    // hardware, while `ua_is_bot` carries the spider signal.
    let (device, brand) = if is_bot {
        (None, None)
    } else {
        (
            norm_family(&client.device.family),
            norm_opt(client.device.brand.as_deref()),
        )
    };

    Parsed {
        browser,
        browser_version,
        os,
        os_version,
        device,
        brand,
        is_bot,
    }
}

/// `ua_browser`: browser/client family (e.g. `"Chrome"`), or `None`.
pub fn browser(ua: &str) -> Option<String> {
    parse(ua).browser
}

/// `ua_browser_version`: dotted browser version (e.g. `"120.0.0"`), or `None`.
pub fn browser_version(ua: &str) -> Option<String> {
    parse(ua).browser_version
}

/// `ua_os`: operating-system family (e.g. `"Windows"`, `"iOS"`), or `None`.
pub fn os(ua: &str) -> Option<String> {
    parse(ua).os
}

/// `ua_os_version`: dotted OS version (e.g. `"10"`, `"17.0"`), or `None`.
pub fn os_version(ua: &str) -> Option<String> {
    parse(ua).os_version
}

/// `ua_device`: device family/model (e.g. `"iPhone"`), or `None`.
pub fn device(ua: &str) -> Option<String> {
    parse(ua).device
}

/// `ua_device_brand`: device brand (e.g. `"Apple"`), or `None`.
pub fn device_brand(ua: &str) -> Option<String> {
    parse(ua).brand
}

/// `ua_is_bot`: `true` if the UA is a spider/crawler (uap device family
/// `"Spider"`).
pub fn is_bot(ua: &str) -> bool {
    parse(bound(ua)).is_bot
}

#[cfg(test)]
mod tests {
    use super::*;

    // Stable, well-known UA strings with assertions that hold across uap-core
    // updates (families, not exact versions).
    const CHROME_WIN: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
        (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
    const IPHONE_SAFARI: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
        AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
    const ANDROID_CHROME: &str = "Mozilla/5.0 (Linux; Android 13; Pixel 7) AppleWebKit/537.36 \
        (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";
    const GOOGLEBOT: &str =
        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";

    #[test]
    fn chrome_on_windows() {
        let p = parse(CHROME_WIN);
        assert_eq!(p.browser.as_deref(), Some("Chrome"));
        assert_eq!(p.os.as_deref(), Some("Windows"));
        assert!(p.browser_version.as_deref().unwrap().starts_with("120"));
        assert!(!p.is_bot);
    }

    #[test]
    fn iphone_safari() {
        let p = parse(IPHONE_SAFARI);
        assert_eq!(p.os.as_deref(), Some("iOS"));
        assert_eq!(p.device.as_deref(), Some("iPhone"));
        assert_eq!(p.brand.as_deref(), Some("Apple"));
        assert!(!p.is_bot);
    }

    #[test]
    fn android_chrome() {
        let p = parse(ANDROID_CHROME);
        assert_eq!(p.os.as_deref(), Some("Android"));
        // Mobile Chrome reports family "Chrome Mobile" in uap-core.
        assert!(p.browser.as_deref().unwrap().starts_with("Chrome"));
        assert_eq!(p.brand.as_deref(), Some("Google"));
        assert!(!p.is_bot);
    }

    #[test]
    fn googlebot_is_a_bot() {
        assert!(is_bot(GOOGLEBOT));
        let p = parse(GOOGLEBOT);
        assert!(p.is_bot);
        // Bot device/brand are suppressed to NULL.
        assert_eq!(p.device, None);
        assert_eq!(p.brand, None);
    }

    #[test]
    fn empty_and_garbage_are_null_not_panic() {
        for ua in ["", "   ", "not a user agent", "%%%###@@@"] {
            let p = parse(ua);
            assert_eq!(p.browser, None, "ua={ua:?}");
            assert_eq!(p.os, None, "ua={ua:?}");
            assert_eq!(p.device, None, "ua={ua:?}");
            assert_eq!(p.brand, None, "ua={ua:?}");
            assert!(!p.is_bot, "ua={ua:?}");
        }
    }

    #[test]
    fn scalar_accessors_match_parse() {
        assert_eq!(browser(CHROME_WIN), Some("Chrome".into()));
        assert_eq!(os(CHROME_WIN), Some("Windows".into()));
        assert_eq!(os(IPHONE_SAFARI), Some("iOS".into()));
        assert_eq!(device(IPHONE_SAFARI), Some("iPhone".into()));
        assert_eq!(device_brand(IPHONE_SAFARI), Some("Apple".into()));
        assert!(is_bot(GOOGLEBOT));
        assert!(!is_bot(CHROME_WIN));
    }

    #[test]
    fn oversized_input_is_bounded_not_panicking() {
        let mut big = String::from(CHROME_WIN);
        big.push_str(&"x".repeat(MAX_INPUT_BYTES * 2));
        // Must not panic; still parses the meaningful prefix.
        let p = parse(&big);
        assert_eq!(p.browser.as_deref(), Some("Chrome"));
    }

    #[test]
    fn version_assembly() {
        assert_eq!(version(&[Some("17"), Some("0"), None]), Some("17.0".into()));
        assert_eq!(version(&[Some("10"), None, None]), Some("10".into()));
        assert_eq!(version(&[None, Some("3")]), None);
        assert_eq!(version(&[]), None);
    }
}
