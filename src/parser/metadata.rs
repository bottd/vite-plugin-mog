use mog_parser::{Data, Value};
use serde_json::{Map, Value as Json, json};

/// The document's `` ``meta: `` block as JSON. KDL nodes map to keys; a node's
/// arguments become its value (one argument a scalar, several an array), and
/// properties or child nodes make it an object.
pub fn extract_metadata(meta: Option<&[Data]>) -> Map<String, Json> {
    meta.map(into_map).unwrap_or_default()
}

fn into_map(data: &[Data]) -> Map<String, Json> {
    data.iter()
        .filter_map(|entry| {
            let name = entry.name.clone()?;
            // Handing this map to Node assigns each key onto a JS object, and
            // assigning `__proto__` swaps the object's prototype instead of
            // adding a key — every other field disappears with it. Dropping the
            // one key keeps the rest of the metadata intact.
            if name == "__proto__" {
                crate::diagnostics::warn(
                    "metadata key \"__proto__\" is not supported and was dropped",
                );
                return None;
            }
            Some((name, into_json(&entry.value)))
        })
        .collect()
}

fn into_json(value: &Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::Bool(bool) => json!(bool),
        Value::Float(float) => json!(float),
        // serde_json numbers top out at 64 bits; a wider KDL integer keeps its
        // exact value as a string rather than silently rounding.
        Value::Int(int) => {
            i64::try_from(*int).map_or_else(|_| json!(int.to_string()), |n| json!(n))
        }
        Value::String(string) => json!(string),
        Value::Node { entries, children } => {
            let mut arguments: Vec<Json> = Vec::new();
            let mut object = into_map(children);
            for entry in entries {
                match &entry.name {
                    Some(name) => {
                        object.insert(name.clone(), into_json(&entry.value));
                    }
                    None => arguments.push(into_json(&entry.value)),
                }
            }

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
        extract_metadata(mog_parser::parse(source).meta.as_deref())
    }

    #[test]
    fn arguments_become_scalars_and_arrays() {
        let meta =
            metadata("``meta:\ntitle \"My Document\"\nauthors \"John\" \"Jane\"\nversion 1\n``\n");
        assert_eq!(meta["title"], json!("My Document"));
        assert_eq!(meta["authors"], json!(["John", "Jane"]));
        assert_eq!(meta["version"], json!(1));
    }

    #[test]
    fn children_and_properties_become_objects() {
        let meta = metadata("``meta:\nauthor name=\"Drake\" {\n  email \"a@b.com\"\n}\n``\n");
        assert_eq!(meta["author"], json!({"name": "Drake", "email": "a@b.com"}));
    }

    #[test]
    fn a_document_without_metadata_is_empty() {
        assert!(metadata("# Heading\n").is_empty());
    }

    #[test]
    fn a_proto_key_is_dropped_without_taking_the_rest_with_it() {
        let meta = metadata("``meta:\ntitle \"Kept\"\n__proto__ \"dropped\"\n``\n");
        assert_eq!(meta["title"], json!("Kept"));
        assert!(!meta.contains_key("__proto__"));
    }
}
