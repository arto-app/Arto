//! The JSON Schema of `config.json`, for editors that validate and complete
//! the file while it is edited by hand.
//!
//! The schema is derived from the configuration types, so it cannot describe
//! a field the app does not read. A copy is committed at
//! `schemas/config.schema.json`, where the URL `config.json` names serves it
//! from; a test keeps that copy in step with the types.

use crate::Config;
use schemars::transform::RecursiveTransform;
use schemars::{JsonSchema, Schema};
use serde::{Deserialize, Serialize};

/// Where the committed schema is served from.
///
/// The default branch rather than a release tag: a URL written into a user's
/// file outlives the version that wrote it, and a tag would keep describing
/// that old version after every upgrade.
pub const SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/arto-app/Arto/main/schemas/config.schema.json";

// A file without one gets `SCHEMA_URL`, so the first save gives the file its
// schema; a file naming another is kept as written, so saving from the
// preferences window never rewrites a line the user chose.
/// Where editors find the JSON Schema of this file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct SchemaUrl(pub String);

impl Default for SchemaUrl {
    fn default() -> Self {
        Self(SCHEMA_URL.to_string())
    }
}

/// The JSON Schema of `config.json`.
pub fn json_schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::draft2020_12()
        .with_transform(RecursiveTransform(deny_unknown_properties))
        .into_generator()
        .into_root_schema_for::<Config>()
}

/// Close every object that lists its properties.
///
/// The types do not deny unknown fields — a stale key in an old file must not
/// stop the app from starting — but the key is still ignored and dropped on
/// the next save, which is what an editor should say about it.
fn deny_unknown_properties(schema: &mut Schema) {
    if schema.get("properties").is_some() && schema.get("additionalProperties").is_none() {
        schema.insert("additionalProperties".to_string(), false.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn committed_schema_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/config.schema.json")
    }

    #[test]
    fn a_file_without_schema_line_gets_the_published_one() {
        let parsed: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed.schema.0, SCHEMA_URL);

        let json = serde_json::to_value(&parsed).unwrap();
        assert_eq!(json["$schema"], SCHEMA_URL);
    }

    #[test]
    fn a_schema_line_the_user_wrote_survives_a_save() {
        let parsed: Config =
            serde_json::from_str(r#"{"$schema": "./my-config.schema.json"}"#).unwrap();

        let json = serde_json::to_value(&parsed).unwrap();
        assert_eq!(json["$schema"], "./my-config.schema.json");
    }

    #[test]
    fn the_schema_accepts_what_arto_writes() {
        let schema = serde_json::to_value(json_schema()).unwrap();
        let written = serde_json::to_value(Config::default()).unwrap();

        let properties = schema["properties"].as_object().unwrap();
        for key in written.as_object().unwrap().keys() {
            assert!(properties.contains_key(key), "schema lacks {key}");
        }
    }

    #[test]
    fn the_schema_flags_keys_arto_does_not_read() {
        // Arto ignores an unknown key and drops it on the next save, so an
        // editor should point it out while it can still be fixed.
        let schema = serde_json::to_value(json_schema()).unwrap();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["$defs"]["ThemeConfig"]["additionalProperties"],
            false
        );
    }

    /// Run with `ARTO_UPDATE_SCHEMA=1` to rewrite the committed copy after
    /// changing a configuration type.
    #[test]
    fn the_committed_schema_matches_the_types() {
        let generated = format!(
            "{}\n",
            serde_json::to_string_pretty(&json_schema()).unwrap()
        );
        let path = committed_schema_path();
        if std::env::var_os("ARTO_UPDATE_SCHEMA").is_some() {
            std::fs::write(&path, &generated).unwrap();
        }
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed == generated,
            "{} is out of date; run `ARTO_UPDATE_SCHEMA=1 cargo test -p arto-config` to regenerate it",
            path.display()
        );
    }
}
