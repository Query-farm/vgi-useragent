//! Shared helpers for the per-object discovery/description metadata that the
//! `vgi-lint` strict profile expects on **every** function.
//!
//! Each function surfaces these in its `FunctionMetadata.tags`:
//! - `vgi.title` (VGI124)    — human-friendly display name
//! - `vgi.doc_llm` (VGI112)  — Markdown narrative aimed at LLMs/agents
//! - `vgi.doc_md` (VGI113)   — Markdown narrative for human docs
//! - `vgi.keywords` (VGI126) — JSON array of search terms/synonyms
//!
//! Per VGI139, `vgi.source_url` lives only on the catalog object, not on every
//! function, so it is intentionally not emitted here.

/// Serialize a comma-separated keyword list into the JSON array of strings that
/// `vgi.keywords` requires (VGI138), e.g. `"a, b"` -> `["a","b"]`. Each keyword
/// is trimmed and empty entries are dropped.
pub fn keywords_json(keywords: &str) -> String {
    let items: Vec<String> = keywords
        .split(',')
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
        .map(|k| serde_json::to_string(k).expect("string serializes to JSON"))
        .collect();
    format!("[{}]", items.join(","))
}

/// Build the standard per-object discovery/description tags.
///
/// `keywords` is a comma-separated convenience list; it is serialized to the
/// JSON array form `vgi.keywords` requires (VGI138). `doc_llm` and `doc_md` MUST
/// be distinct Markdown narratives (identical content is flagged as
/// duplication). `category` names one of the schema's `vgi.categories` entries
/// (VGI413) and is emitted as the object's `vgi.category`. Note:
/// `vgi.source_url` is deliberately omitted here — per VGI139 it belongs only on
/// the catalog object.
pub fn object_tags(
    title: &str,
    doc_llm: &str,
    doc_md: &str,
    keywords: &str,
    category: &str,
) -> Vec<(String, String)> {
    vec![
        ("vgi.title".to_string(), title.to_string()),
        ("vgi.doc_llm".to_string(), doc_llm.to_string()),
        ("vgi.doc_md".to_string(), doc_md.to_string()),
        ("vgi.keywords".to_string(), keywords_json(keywords)),
        ("vgi.category".to_string(), category.to_string()),
    ]
}
