//! O1-KR1 e O1-KR4 per il contesto Notebook Governance: il manifest autoritativo.

use kubirag_acceptance::golden;
use kubirag_domain::notebook::stable_notebook_id;
use kubirag_store_json::{ManifestCodec, ManifestReadResult};

#[test]
fn golden_manifests_round_trip_byte_exact() {
    let divergences = ["empty", "rich", "real-kubirag"]
        .into_iter()
        .filter_map(|label| {
            let file = format!("notebook/manifests/{label}-manifest.json");
            let text = golden::read(&file);
            let ManifestReadResult::Current(manifest) = ManifestCodec::classify(Some(&text)) else {
                return Some(format!("{file}: non classificato come Current"));
            };
            let written = ManifestCodec::encode(&manifest).unwrap();
            // Il writer Kotlin non mette il newline finale; il file reale committato in kubirag
            // ne ha uno, aggiunto da un editor. Conta il contenuto scritto dal writer.
            (written != text.strip_suffix('\n').unwrap_or(&text))
                .then(|| format!("{file}\n--- kotlin\n{text}\n--- rust\n{written}"))
        })
        .collect();
    golden::assert_no_divergence("manifest round-trip", divergences);
}

fn kind(result: &ManifestReadResult) -> &'static str {
    match result {
        ManifestReadResult::Current(_) => "Current",
        ManifestReadResult::LegacyMissingIdentity { .. } => "LegacyMissingIdentity",
        ManifestReadResult::UnsupportedVersion(_) => "UnsupportedVersion",
        ManifestReadResult::Invalid(_) => "Invalid",
        ManifestReadResult::Missing => "Missing",
    }
}

#[test]
fn read_classification_matches_kotlin() {
    let vectors = golden::json("notebook/read-classification.json");
    let divergences = vectors["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| {
            let result = ManifestCodec::classify(v["input"].as_str());
            let expected = v["expected"].as_str().unwrap();
            let version_ok = match (&result, v["detail"].as_str()) {
                (ManifestReadResult::UnsupportedVersion(actual), Some(detail)) => actual == detail,
                _ => true,
            };
            (kind(&result) != expected || !version_ok)
                .then(|| format!("{}: atteso {expected}, ottenuto {result:?}", v["case"]))
        })
        .collect();
    golden::assert_no_divergence("read-classification", divergences);
}

#[test]
fn missing_file_is_classified_as_missing() {
    assert_eq!(ManifestCodec::classify(None), ManifestReadResult::Missing);
}

#[test]
fn stable_notebook_ids_match_kotlin() {
    let vectors = golden::json("notebook/stable-notebook-ids.json");
    let divergences = vectors["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| {
            let actual = stable_notebook_id(v["name"].as_str().unwrap());
            (actual.as_str() != v["expected"].as_str().unwrap()).then(|| format!("{v} -> {actual}"))
        })
        .collect();
    golden::assert_no_divergence("stable-notebook-ids", divergences);
}
