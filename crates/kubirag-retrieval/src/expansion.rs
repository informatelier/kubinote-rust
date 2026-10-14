//! `QueryExpansionService`: i token italiani della query più i sinonimi del dizionario.
//! I sinonimi si cercano sul token già analizzato (stemmato), come in Kotlin: `database`
//! diventa `databas` e non trova la voce `database`.

use indexmap::IndexSet;

use crate::analysis::Analyzer;

const SYNONYMS: [(&str, &[&str]); 24] = [
    ("kotlin", &["kt", "kotlin language", "linguaggio kotlin"]),
    ("java", &["jvm", "linguaggio java"]),
    ("programmazione", &["coding", "sviluppo", "development"]),
    ("codice", &["sorgente", "source", "programma"]),
    ("linguaggio", &["parlato", "lingua", "language", "idioma"]),
    ("parola", &["termine", "vocabolo", "word"]),
    ("testo", &["documento", "contenuto", "text"]),
    ("documento", &["file", "archivio", "testo"]),
    ("sistema", &["software", "applicazione", "programma"]),
    ("funzione", &["metodo", "routine", "function"]),
    ("classe", &["oggetto", "type", "tipo"]),
    ("variabile", &["campo", "attributo", "property"]),
    ("ricerca", &["search", "query", "interrogazione"]),
    ("indice", &["index", "catalogo", "archivio"]),
    ("risultato", &["output", "risposta", "result"]),
    ("embedding", &["vettore", "vector", "rappresentazione"]),
    ("database", &["db", "dati", "storage"]),
    ("server", &["servizio", "host", "backend"]),
    ("client", &["frontend", "utente", "applicazione client"]),
    ("api", &["interfaccia", "endpoint", "servizio"]),
    ("architettura", &["design", "struttura", "pattern"]),
    ("microservizio", &["servizio", "componente", "modulo"]),
    ("container", &["docker", "pod", "runtime"]),
    ("cloud", &["nuvola", "saas", "paas"]),
];

fn synonyms(term: &str) -> &'static [&'static str] {
    SYNONYMS
        .iter()
        .find(|(key, _)| *key == term)
        .map_or(&[], |(_, values)| values)
}

/// `expandQuery(query, italianAnalyzer)`: token, sinonimi e "termini correlati" (oggi gli
/// stessi sinonimi), senza duplicati, uniti da uno spazio.
pub fn expand_query(query: &str) -> String {
    let mut expanded: IndexSet<String> = IndexSet::new();
    for token in Analyzer::Italian.terms(query) {
        let lower = crate::analysis::lowercase(&token);
        let related = synonyms(&lower);
        expanded.insert(token);
        for synonym in related.iter().chain(related) {
            expanded.insert((*synonym).to_owned());
        }
    }
    expanded.into_iter().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synonyms_are_looked_up_on_the_stemmed_token() {
        assert_eq!(
            expand_query("Kotlin database"),
            "kotlin kt kotlin language linguaggio kotlin databas"
        );
        assert_eq!(expand_query("il lo la di"), "");
    }
}
