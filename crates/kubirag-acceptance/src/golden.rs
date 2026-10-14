//! Accesso ai golden file del repository.

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

pub fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts/golden")
}

pub fn read(relative: &str) -> String {
    let path = dir().join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read golden {}: {e}", path.display()))
}

pub fn json(relative: &str) -> Value {
    serde_json::from_str(&read(relative))
        .unwrap_or_else(|e| panic!("invalid golden {relative}: {e}"))
}

/// Fallisce elencando tutte le divergenze, non solo la prima.
pub fn assert_no_divergence(contract: &str, divergences: Vec<String>) {
    assert!(
        divergences.is_empty(),
        "{contract}: {} divergenze dalla versione Kotlin\n{}",
        divergences.len(),
        divergences.join("\n")
    );
}
