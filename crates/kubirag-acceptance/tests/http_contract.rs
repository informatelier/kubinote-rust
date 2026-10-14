//! O3-KR1: le route chiamate dalla SPA sono il contratto del server Rust (F3).
//! In F0 si verifica che l'inventario sia coerente con l'OpenAPI pubblicato da kubirag.

use kubirag_acceptance::golden;

#[test]
fn openapi_contract_is_present() {
    let spec = golden::json("http/openapi.json");
    assert_eq!(spec["openapi"], "3.0.3");
    assert!(spec["paths"].as_object().unwrap().len() >= 100);
}

#[test]
fn spa_routes_are_documented_or_static() {
    let inventory = golden::json("http/spa-routes.json");
    let undocumented: Vec<&str> = inventory["undocumented"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap())
        .collect();
    // Unico residuo noto: componenti HTML caricati a runtime, asset statici da incorporare.
    assert_eq!(undocumented, ["/web/components/{}"]);
}
