//! # kubirag-acceptance
//!
//! Suite di accettazione del porting (fase F0). I golden in `contracts/golden` sono prodotti
//! dalla versione Kotlin (`contracts/jvm-generator`); i test in `tests/` li usano per
//! verificare la versione Rust, contesto per contesto. Qui stanno solo gli strumenti comuni.

pub mod fake_ollama;
pub mod golden;
pub mod http;
pub mod openapi;
pub mod retrieval;
pub mod wire;
