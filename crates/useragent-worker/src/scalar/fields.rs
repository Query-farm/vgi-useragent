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
use vgi::{ArgSpec, BindParams, BindResponse, FunctionMetadata, ProcessParams, ScalarFunction};
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
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::any_column("ua", 0, "User-Agent string (VARCHAR)")]
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
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::any_column("ua", 0, "User-Agent string (VARCHAR)")]
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
