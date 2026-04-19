use std::collections::BTreeMap;

use regex::Regex;

use crate::verify::types::ManifestMetadata;

pub fn parse_manifest(input: &str) -> Result<ManifestMetadata, String> {
    let regex = Regex::new(r#""(?P<key>[^"]+)"\s+"(?P<value>[^"]*)""#)
        .map_err(|err| format!("failed to build manifest regex: {err}"))?;
    let values: BTreeMap<String, String> = regex
        .captures_iter(input)
        .map(|caps| (caps["key"].to_string(), caps["value"].to_string()))
        .collect();

    if values.is_empty() {
        return Err("manifest did not contain any key/value pairs".to_string());
    }

    Ok(ManifestMetadata {
        install_root: values.get("installdir").cloned(),
        steam_buildid: values.get("buildid").cloned(),
        last_updated: values.get("LastUpdated").cloned(),
        last_played: values.get("LastPlayed").cloned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_manifest() {
        let manifest = r#""AppState"
{
    "appid" "1091500"
    "installdir" "Cyberpunk 2077"
    "buildid" "20383525"
    "LastUpdated" "1775627166"
    "LastPlayed" "1775691532"
}"#;
        let parsed = parse_manifest(manifest).unwrap();
        assert_eq!(parsed.install_root.as_deref(), Some("Cyberpunk 2077"));
        assert_eq!(parsed.steam_buildid.as_deref(), Some("20383525"));
    }

    #[test]
    fn rejects_malformed_manifest() {
        let err = parse_manifest("not a manifest").unwrap_err();
        assert!(err.contains("did not contain"));
    }
}
