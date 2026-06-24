//! `useragent_version()` — return the worker's version string.

use std::sync::Arc;

use arrow_array::{ArrayRef, RecordBatch, StringArray};
use arrow_schema::DataType;
use vgi::{
    ArgSpec, BindParams, BindResponse, FunctionExample, FunctionMetadata, ProcessParams,
    ScalarFunction,
};
use vgi_rpc::{Result, RpcError};

pub struct UseragentVersion;

impl ScalarFunction for UseragentVersion {
    fn name(&self) -> &str {
        "useragent_version"
    }

    fn metadata(&self) -> FunctionMetadata {
        FunctionMetadata {
            description: "Returns the useragent worker version string".into(),
            return_type: Some(DataType::Utf8),
            examples: vec![FunctionExample {
                sql: "SELECT useragent.main.useragent_version();".into(),
                description: "Return the useragent worker version string.".into(),
                expected_output: None,
            }],
            tags: crate::meta::object_tags(
                "User-Agent Worker Version",
                "## useragent_version\n\nReturns the **semantic version string** of the running \
                 `useragent` worker binary as a `VARCHAR`.\n\n**When to use:** confirm which \
                 build of the parser (and the embedded uap-core regex database) is attached, \
                 capture provenance in audit logs, or assert a minimum version before relying on \
                 a feature.\n\n**Input:** none — this is a zero-argument scalar.\n**Output:** a \
                 version string such as `0.1.0`, taken from the crate's `CARGO_PKG_VERSION` at \
                 compile time.\n\n**Edge cases:** always returns a value; it never depends on \
                 input and never returns `NULL`.",
                "# Worker version\n\n`useragent_version()` returns the worker binary's semantic \
                 version.\n\n## Usage\n\n```sql\nSELECT useragent_version();\n-- '0.1.0'\n```\n\n\
                 ## Notes\n\n- Zero-argument scalar; always non-NULL.\n- Useful for diagnostics \
                 and provenance.",
                "version, useragent_version, worker version, build, semver, diagnostics",
                "scalar/version.rs",
            ),
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        Vec::new()
    }

    fn on_bind(&self, _params: &BindParams) -> Result<BindResponse> {
        Ok(BindResponse::result(DataType::Utf8))
    }

    fn process(&self, params: &ProcessParams, batch: &RecordBatch) -> Result<RecordBatch> {
        let rows = batch.num_rows();
        let out: ArrayRef = Arc::new(StringArray::from(vec![crate::version(); rows]));
        RecordBatch::try_new(params.output_schema.clone(), vec![out])
            .map_err(|e| RpcError::runtime_error(e.to_string()))
    }
}
