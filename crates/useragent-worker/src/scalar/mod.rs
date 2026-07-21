//! Scalar functions exposed by the useragent worker, registered under
//! `useragent.main`.

mod fields;
mod parse;

use vgi::Worker;

/// Register every scalar function on the worker.
pub fn register(worker: &mut Worker) {
    // Single-field VARCHAR accessors.
    worker.register_scalar(fields::UaField::browser());
    worker.register_scalar(fields::UaField::browser_version());
    worker.register_scalar(fields::UaField::os());
    worker.register_scalar(fields::UaField::os_version());
    worker.register_scalar(fields::UaField::device());
    worker.register_scalar(fields::UaField::device_brand());

    // Bot predicate.
    worker.register_scalar(fields::UaIsBot);

    // One-shot STRUCT parse.
    worker.register_scalar(parse::UaParse);
}
