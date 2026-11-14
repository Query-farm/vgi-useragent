//! Composed example queries: realistic analytics queries that combine two or
//! more of the worker's scalars over a small inline request log.
//!
//! Each example is defined exactly once here and referenced from both the
//! schema-level `vgi.example_queries` list (`lib.rs`) and the example list of
//! the scalar that drives the query (`scalar/fields.rs`), so the two copies can
//! never drift. Every query is offline and deterministic (inline `VALUES`).

/// A described example query (`{description, sql}` in `vgi.example_queries`).
pub struct Example {
    pub description: &'static str,
    pub sql: &'static str,
}

impl Example {
    /// The `{description, sql}` JSON object for this example.
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({ "description": self.description, "sql": self.sql })
    }
}

/// `ua_is_bot` filter feeding a `ua_browser` breakdown: top browsers among
/// human visitors, with the Googlebot and Bingbot crawler hits excluded.
pub const HUMAN_BROWSER_SHARE: Example = Example {
    description: "Web-analytics browser breakdown: filter crawlers out of a request log with ua_is_bot, then count the remaining human requests per ua_browser family (Googlebot and Bingbot are dropped; Chrome 2, Mobile Safari 1).",
    sql: "SELECT useragent.main.ua_browser(ua) AS browser, count(*) AS requests FROM (VALUES ('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'), ('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36'), ('Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1'), ('Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)'), ('Mozilla/5.0 (compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm)')) AS hits(ua) WHERE NOT useragent.main.ua_is_bot(ua) GROUP BY browser ORDER BY requests DESC, browser",
};

/// `ua_browser` + `ua_browser_version`: find requests from outdated Chrome.
pub const OUTDATED_CHROME: Example = Example {
    description: "Find visitors on an outdated Chrome: keep rows whose ua_browser is 'Chrome' and whose ua_browser_version major number is below 110 (returns the Chrome 96 request with version '96.0.4664'; Chrome 120 and Firefox are excluded).",
    sql: "SELECT ua, useragent.main.ua_browser_version(ua) AS chrome_version FROM (VALUES ('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'), ('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/96.0.4664.110 Safari/537.36'), ('Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0')) AS hits(ua) WHERE useragent.main.ua_browser(ua) = 'Chrome' AND TRY_CAST(split_part(useragent.main.ua_browser_version(ua), '.', 1) AS INTEGER) < 110 ORDER BY ua",
};

/// `ua_os` filter + `ua_os_version` grouping: iOS version adoption.
pub const IOS_VERSION_ADOPTION: Example = Example {
    description: "iOS version adoption: keep requests whose ua_os is 'iOS' (iPhone and iPad alike, Android dropped) and count them per ua_os_version (16.6 → 1, 17.0 → 2).",
    sql: "SELECT useragent.main.ua_os_version(ua) AS ios_version, count(*) AS requests FROM (VALUES ('Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1'), ('Mozilla/5.0 (iPhone; CPU iPhone OS 16_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Mobile/15E148 Safari/604.1'), ('Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1'), ('Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36')) AS hits(ua) WHERE useragent.main.ua_os(ua) = 'iOS' GROUP BY ios_version ORDER BY ios_version",
};

/// `ua_device_brand` + `ua_device`: hardware breakdown of a request log.
pub const DEVICE_BRAND_MODELS: Example = Example {
    description: "Hardware breakdown: count requests per ua_device_brand and ua_device model, skipping traffic with no identifiable brand such as a generic Windows desktop (Apple iPhone, Google Pixel 7, Samsung SM-S911B; 1 request each).",
    sql: "SELECT useragent.main.ua_device_brand(ua) AS brand, useragent.main.ua_device(ua) AS device, count(*) AS requests FROM (VALUES ('Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1'), ('Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36'), ('Mozilla/5.0 (Linux; Android 13; SM-S911B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36'), ('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36')) AS hits(ua) WHERE useragent.main.ua_device_brand(ua) IS NOT NULL GROUP BY brand, device ORDER BY brand, device",
};

/// Every composed example, in schema-level listing order.
pub const ALL: [&Example; 4] = [
    &HUMAN_BROWSER_SHARE,
    &OUTDATED_CHROME,
    &IOS_VERSION_ADOPTION,
    &DEVICE_BRAND_MODELS,
];

/// Append `extra` example objects to a JSON-list tag value `base` (a
/// `[{description, sql}, …]` string), returning the combined JSON list.
pub fn append_examples(base: &str, extra: &[&Example]) -> String {
    let mut list: Vec<serde_json::Value> =
        serde_json::from_str(base).expect("example list constant is valid JSON");
    list.extend(extra.iter().map(|e| e.json()));
    serde_json::Value::Array(list).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composed_examples_are_catalog_qualified_and_ordered() {
        for e in ALL {
            assert!(!e.description.is_empty());
            assert!(e.sql.contains("useragent.main."), "{}", e.sql);
            assert!(e.sql.contains("ORDER BY"), "{}", e.sql);
            assert!(!e.sql.contains("SELECT *"), "{}", e.sql);
            // Two or more distinct worker functions per composed example.
            let fns = [
                "ua_browser(",
                "ua_browser_version(",
                "ua_os(",
                "ua_os_version(",
                "ua_device(",
                "ua_device_brand(",
                "ua_is_bot(",
                "ua_parse(",
            ];
            let used = fns
                .iter()
                .filter(|f| e.sql.contains(&format!("useragent.main.{f}")))
                .count();
            assert!(used >= 2, "{}", e.sql);
        }
    }

    #[test]
    fn append_keeps_base_and_adds_extra() {
        let out = append_examples(r#"[{"description":"d","sql":"s"}]"#, &[&OUTDATED_CHROME]);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[1]["sql"], OUTDATED_CHROME.sql);
    }
}
