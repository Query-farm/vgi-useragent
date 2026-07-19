//! Small Arrow helpers shared across the scalar functions: reading VARCHAR input
//! cells and constructing the `ua_parse` output `STRUCT` type in a way that
//! `on_bind` (declared output schema) and `process` (the array actually built)
//! agree on exactly. The in-process test harness below drives a `ScalarFunction`
//! end-to-end without the RPC/IPC plumbing.

use arrow_schema::{DataType, Field, Fields};
use vgi_rpc::{Result, RpcError};

/// Borrow the UTF-8 text of a VARCHAR cell at `row`, or `None` if null. Errors if
/// the column isn't a string type.
pub fn text_str(col: &arrow_array::ArrayRef, row: usize) -> Result<Option<&str>> {
    use arrow_array::cast::AsArray;
    use arrow_array::Array;
    if col.is_null(row) {
        return Ok(None);
    }
    Ok(Some(match col.data_type() {
        DataType::Utf8 => col.as_string::<i32>().value(row),
        DataType::LargeUtf8 => col.as_string::<i64>().value(row),
        other => {
            return Err(RpcError::value_error(format!(
                "expected a VARCHAR (string) argument, got {other:?}"
            )))
        }
    }))
}

/// The fixed `ua_parse` output STRUCT fields. This MUST match between `on_bind`
/// (declared schema) and `process` (built [`arrow_array::StructArray`]), or
/// DuckDB rejects the batch.
pub fn parse_struct_fields() -> Fields {
    Fields::from(vec![
        Field::new("browser", DataType::Utf8, true),
        Field::new("browser_version", DataType::Utf8, true),
        Field::new("os", DataType::Utf8, true),
        Field::new("os_version", DataType::Utf8, true),
        Field::new("device", DataType::Utf8, true),
        Field::new("brand", DataType::Utf8, true),
        Field::new("is_bot", DataType::Boolean, true),
    ])
}

/// The `ua_parse` return `DataType` (a `STRUCT`).
pub fn parse_struct_type() -> DataType {
    DataType::Struct(parse_struct_fields())
}

/// Test-only helpers shared by the scalar Arrow-boundary unit tests: build a
/// one-column VARCHAR input `RecordBatch`, run `on_bind` + `process`, and inspect
/// the result — all in-process, no RPC/IPC.
#[cfg(test)]
pub mod test_support {
    use std::sync::Arc;

    use arrow_array::builder::StringBuilder;
    use arrow_array::{ArrayRef, RecordBatch};
    use arrow_schema::{Field, Schema, SchemaRef};
    use vgi::arguments::Arguments;
    use vgi::{BindParams, ProcessParams, ScalarFunction};
    use vgi_rpc::Result;

    /// A single-column `Utf8` (VARCHAR) input batch. `None` entries become NULLs.
    pub fn text_batch(rows: &[Option<&str>]) -> RecordBatch {
        let mut b = StringBuilder::new();
        for r in rows {
            match r {
                Some(s) => b.append_value(s),
                None => b.append_null(),
            }
        }
        let arr: ArrayRef = Arc::new(b.finish());
        let schema = Arc::new(Schema::new(vec![Field::new(
            "ua",
            arr.data_type().clone(),
            true,
        )]));
        RecordBatch::try_new(schema, vec![arr]).unwrap()
    }

    /// Build a `ProcessParams` carrying the given output schema and arguments.
    pub fn process_params(output_schema: SchemaRef, arguments: Arguments) -> ProcessParams {
        ProcessParams {
            substream_id: None,
            if_none_match: None,
            if_modified_since: None,
            output_schema,
            input_schema: None,
            execution_id: Vec::new(),
            init_opaque_data: Vec::new(),
            arguments,
            settings: Default::default(),
            secrets: Default::default(),
            auth_principal: None,
            projection_ids: None,
            pushdown_filters: None,
            join_keys: Vec::new(),
            storage: None,
            order_by_column: None,
            order_by_direction: None,
            order_by_null_order: None,
            order_by_limit: None,
            tablesample_percentage: None,
            tablesample_seed: None,
            attach_opaque_data: None,
            at_unit: None,
            at_value: None,
            copy_from: None,
        }
    }

    /// Run a scalar function over a prebuilt input batch: call `on_bind` to obtain
    /// the declared output schema, then `process`, returning the single result
    /// column.
    pub fn run_scalar_on<F: ScalarFunction>(
        f: &F,
        batch: RecordBatch,
        arguments: Arguments,
    ) -> Result<ArrayRef> {
        let bind = BindParams {
            input_schema: Some(batch.schema()),
            arguments: arguments.clone(),
            ..Default::default()
        };
        let bound = f.on_bind(&bind)?;
        let params = process_params(bound.output_schema.clone(), arguments);
        let out = f.process(&params, &batch)?;
        Ok(out.column(0).clone())
    }

    /// Run a scalar over a single-column VARCHAR input batch.
    pub fn run_scalar_text<F: ScalarFunction>(
        f: &F,
        rows: &[Option<&str>],
        arguments: Arguments,
    ) -> Result<ArrayRef> {
        run_scalar_on(f, text_batch(rows), arguments)
    }

    /// The declared output `DataType` from `on_bind` for a scalar with no
    /// bind-time argument requirements.
    pub fn bound_type<F: ScalarFunction>(f: &F) -> arrow_schema::DataType {
        let bind = BindParams::default();
        let bound = f.on_bind(&bind).unwrap();
        bound.output_schema.field(0).data_type().clone()
    }
}
