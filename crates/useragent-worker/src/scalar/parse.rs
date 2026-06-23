//! `ua_parse(ua)` → `STRUCT(browser VARCHAR, browser_version VARCHAR,
//! os VARCHAR, os_version VARCHAR, device VARCHAR, brand VARCHAR,
//! is_bot BOOLEAN)`. A one-shot parse: every field computed from a single pass
//! over the UA string. NULL / empty / unparseable input → a NULL struct row.

use std::sync::Arc;

use arrow_array::builder::{BooleanBuilder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch, StructArray};
use arrow_buffer::NullBuffer;
use vgi::{ArgSpec, BindParams, BindResponse, FunctionMetadata, ProcessParams, ScalarFunction};
use vgi_rpc::{Result, RpcError};

use crate::arrow_io::{parse_struct_fields, parse_struct_type, text_str};
use crate::useragent;

pub struct UaParse;

impl ScalarFunction for UaParse {
    fn name(&self) -> &str {
        "ua_parse"
    }

    fn metadata(&self) -> FunctionMetadata {
        FunctionMetadata {
            description: "Parse a User-Agent into STRUCT(browser, browser_version, os, \
                          os_version, device, brand, is_bot); NULL/unparseable → NULL row"
                .into(),
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
