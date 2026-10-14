//! # kubirag-store-json
//!
//! Adattatore di persistenza compatibile con i file della versione Kotlin (strangler fig:
//! le due versioni leggono e scrivono gli stessi notebook). In F0 copre la codifica del
//! manifest autoritativo; lettura e scrittura su disco con lock e CAS arrivano in F1.

mod manifest_codec;

pub use manifest_codec::{ManifestCodec, ManifestReadResult};
