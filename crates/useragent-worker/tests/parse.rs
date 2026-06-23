//! Integration tests: black-box exercise of the worker's pure UA parsing logic
//! over well-known User-Agent strings, the same way the SQL E2E suite drives it
//! but without the Arrow/RPC layer.
//!
//! The pure logic lives in a private module of the binary crate, so we include
//! it by path — the same trick `vgi-ioc` / `vgi-barcode` use for integration
//! tests.

#[path = "../src/useragent.rs"]
#[allow(dead_code)]
mod useragent;

const CHROME_WIN: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const IPHONE_SAFARI: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
    AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
const ANDROID_CHROME: &str = "Mozilla/5.0 (Linux; Android 13; Pixel 7) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";
const GOOGLEBOT: &str = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";

#[test]
fn chrome_on_windows() {
    let p = useragent::parse(CHROME_WIN);
    assert_eq!(p.browser.as_deref(), Some("Chrome"));
    assert_eq!(p.os.as_deref(), Some("Windows"));
    assert!(!p.is_bot);
}

#[test]
fn iphone_safari_is_ios_iphone() {
    let p = useragent::parse(IPHONE_SAFARI);
    assert_eq!(p.os.as_deref(), Some("iOS"));
    assert_eq!(p.device.as_deref(), Some("iPhone"));
    assert_eq!(p.brand.as_deref(), Some("Apple"));
    assert!(!p.is_bot);
}

#[test]
fn android_chrome() {
    let p = useragent::parse(ANDROID_CHROME);
    assert_eq!(p.os.as_deref(), Some("Android"));
    assert!(p.browser.as_deref().unwrap().starts_with("Chrome"));
    assert_eq!(p.brand.as_deref(), Some("Google"));
    assert!(!p.is_bot);
}

#[test]
fn googlebot_is_bot() {
    assert!(useragent::is_bot(GOOGLEBOT));
    let p = useragent::parse(GOOGLEBOT);
    assert!(p.is_bot);
    assert_eq!(p.device, None);
}

#[test]
fn empty_and_garbage_are_null_not_panic() {
    for ua in ["", "garbage", "%%%%"] {
        let p = useragent::parse(ua);
        assert_eq!(p.browser, None);
        assert_eq!(p.os, None);
        assert_eq!(p.device, None);
        assert!(!p.is_bot);
    }
}

#[test]
fn parse_struct_fields_one_shot() {
    let p = useragent::parse(IPHONE_SAFARI);
    assert_eq!(p.os.as_deref(), Some("iOS"));
    assert!(p.os_version.is_some());
    assert!(p.browser.is_some());
    assert!(p.browser_version.is_some());
    assert_eq!(p.device.as_deref(), Some("iPhone"));
    assert_eq!(p.brand.as_deref(), Some("Apple"));
    assert!(!p.is_bot);
}
