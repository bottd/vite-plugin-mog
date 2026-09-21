use mog_parser::{Attribute, Attributes, Value};
use serde_json::{Map, Value as Json, json};

/// The document's root-level `` ``attr: `` blocks as JSON, merged in source
/// order. Such a block is a KDL document, so it arrives as the document
/// attributes' *children* — one node per key, never as chain `entries`.
///
/// Each node maps to a key; its arguments become the value (one argument a
/// scalar, several an array), and properties or child nodes make it an object.
///
/// A key set more than once keeps its first value, at any depth: blocks merge
/// from anywhere at the root, so a later one is as likely a stray sample as an
/// override.
pub fn extract_metadata(attributes: Option<&Attributes>) -> Map<String, Json> {
    let Some(attributes) = attributes else {
        return Map::new();
    };
    // Nothing spells a chain on the document today. A warning rather than an
    // assert: this runs on user input, in debug and release builds alike.
    if !attributes.entries.is_empty() {
        crate::diagnostics::warn(
            "document attributes carry chain entries, which metadata does not read — \
             they were dropped",
        );
    }

    into_map(&attributes.children)
}

pub(crate) fn into_map(attributes: &[Attribute]) -> Map<String, Json> {
    let mut map = Map::new();
    extend_once(&mut map, attributes);
    map
}

fn extend_once(map: &mut Map<String, Json>, attributes: &[Attribute]) {
    for entry in attributes {
        if let Some(name) = &entry.name {
            insert_once(map, name, &entry.value);
        }
    }
}

fn insert_once(map: &mut Map<String, Json>, name: &str, value: &Value) {
    // Handing this map to Node assigns each key onto a JS object, and assigning
    // `__proto__` swaps the object's prototype instead of adding a key — every
    // other field disappears with it. Dropping the one key keeps the rest intact.
    if name == "__proto__" {
        crate::diagnostics::warn("attr key \"__proto__\" is not supported and was dropped");
    } else if map.contains_key(name) {
        crate::diagnostics::warn(format!(
            "attr key \"{name}\" is set more than once — the first value was kept"
        ));
    } else {
        map.insert(name.to_string(), into_json(value));
    }
}

/// JavaScript's exact-integer ceiling. A KDL integer beyond it cannot cross as
/// a number without losing its low digits — `i64` is not the bound here, because
/// what receives these is a JS number, not an `i64`.
const SAFE_INTEGER: i128 = 9_007_199_254_740_991;

/// The integer as a JS-safe number, if representable. Both metadata and the AST
/// use this bound and otherwise preserve the digits as a string.
pub(crate) fn safe_integer(int: i128) -> Option<i64> {
    (int.unsigned_abs() <= SAFE_INTEGER as u128).then_some(int as i64)
}

pub(crate) fn into_json(value: &Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::Bool(bool) => json!(bool),
        Value::Float(float) => json!(float),
        Value::Int(int) => safe_integer(*int).map_or_else(|| json!(int.to_string()), |n| json!(n)),
        Value::String(string) => json!(string),
        Value::Node(node) => {
            let mut arguments: Vec<Json> = Vec::new();
            let mut object = Map::new();
            // Properties sit on the node's own line, ahead of its children, so
            // they go in first: the first value in source is the one kept.
            for entry in &node.entries {
                match &entry.name {
                    Some(name) => insert_once(&mut object, name, &entry.value),
                    None => arguments.push(into_json(&entry.value)),
                }
            }
            extend_once(&mut object, &node.children);

            if object.is_empty() {
                return match arguments.len() {
                    0 => Json::Null,
                    1 => arguments.remove(0),
                    _ => Json::Array(arguments),
                };
            }
            if !arguments.is_empty() {
                object.insert("args".to_string(), Json::Array(arguments));
            }
            Json::Object(object)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(source: &str) -> Map<String, Json> {
        extract_metadata(mog_parser::parse(source).attributes.as_deref())
    }

    #[test]
    fn arguments_become_scalars_and_arrays() {
        let meta =
            metadata("``attr:\ntitle \"My Document\"\nauthors \"John\" \"Jane\"\nversion 1\n``\n");
        assert_eq!(meta["title"], json!("My Document"));
        assert_eq!(meta["authors"], json!(["John", "Jane"]));
        assert_eq!(meta["version"], json!(1));
    }

    #[test]
    fn children_and_properties_become_objects() {
        let meta = metadata("``attr:\nauthor name=\"Drake\" {\n  email \"a@b.com\"\n}\n``\n");
        assert_eq!(meta["author"], json!({"name": "Drake", "email": "a@b.com"}));
    }

    #[test]
    fn every_root_level_block_merges_into_one_map() {
        let meta =
            metadata("``attr:\ntitle \"First\"\n``\n\n# Heading\n\n``attr:\nversion 2\n``\n");
        assert_eq!(meta["title"], json!("First"));
        assert_eq!(meta["version"], json!(2));
    }

    #[test]
    fn a_repeated_key_keeps_its_first_value_and_warns() {
        let (meta, warnings) = crate::diagnostics::capture(|| {
            metadata(
                "``attr:\ntitle \"Guide\"\n``\n\n# Heading\n\n``attr:\ntitle \"Example\"\n``\n",
            )
        });
        assert_eq!(meta["title"], json!("Guide"));
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("\"title\""), "{warnings:?}");
    }

    #[test]
    fn a_repeated_nested_key_follows_the_same_rule() {
        let (meta, warnings) = crate::diagnostics::capture(|| {
            metadata("``attr:\nauthor name=\"P\" {\n  name \"C\"\n  name \"D\"\n}\n``\n")
        });
        assert_eq!(meta["author"], json!({"name": "P"}));
        assert_eq!(warnings.len(), 2, "{warnings:?}");
    }

    #[test]
    fn chain_entries_on_the_document_warn_rather_than_panic() {
        let attributes = Attributes {
            entries: vec![Attribute {
                name: None,
                ty: None,
                value: Value::String("stray".to_string()),
            }],
            children: vec![Attribute {
                name: Some("title".to_string()),
                ty: None,
                value: Value::String("Kept".to_string()),
            }],
            blocks: Vec::new(),
        };
        let (meta, warnings) = crate::diagnostics::capture(|| extract_metadata(Some(&attributes)));
        assert_eq!(meta["title"], json!("Kept"));
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn an_integer_javascript_cannot_hold_exactly_crosses_as_its_digits() {
        // BREAKING (0.3.0): this used to be bounded by i64, so anything up to
        // 2^63 crossed as a JSON number — and then lost its low digits on the
        // way into a JS number. metadata and the AST now share one rule.
        let meta = metadata("``attr:\nsmall 9007199254740991\nbig 9007199254740993\n``\n");

        assert_eq!(meta["small"], json!(9_007_199_254_740_991i64));
        assert_eq!(meta["big"], json!("9007199254740993"));
    }

    #[test]
    fn a_negative_integer_follows_the_same_bound() {
        let meta = metadata("``attr:\nsmall -9007199254740991\nbig -9007199254740993\n``\n");

        assert_eq!(meta["small"], json!(-9_007_199_254_740_991i64));
        assert_eq!(meta["big"], json!("-9007199254740993"));
    }

    #[test]
    fn a_document_without_metadata_is_empty() {
        assert!(metadata("# Heading\n").is_empty());
    }

    #[test]
    fn a_proto_key_is_dropped_without_taking_the_rest_with_it() {
        let meta = metadata("``attr:\ntitle \"Kept\"\n__proto__ \"dropped\"\n``\n");
        assert_eq!(meta["title"], json!("Kept"));
        assert!(!meta.contains_key("__proto__"));
    }
}
