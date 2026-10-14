//! Bounded context **Concept Knowledge**: grafo autoritativo dei concetti (`qb-concepts.json`).
//!
//! In F1 c'è la forma canonica con cui kubirag scrive il grafo e la sua validazione
//! (`AuthoritativeConceptGraphRepository.canonical` / `validate`): bastano per il round-trip
//! del file. I comandi sui concetti (alias, relazioni, promozione delle evidenze) arrivano in F4.

use std::collections::{HashMap, HashSet};

use indexmap::IndexSet;
use kubi_kernel::domain::concept::{
    AuthorityConceptEvidence, AuthorityConceptGraphFile, AuthorityEvidenceOrigin, GovernedConcept,
};
use kubi_kernel::identity::is_jvm_uuid;

/// Grafo non valido: tutte le diagnostiche, ordinate, come `ConceptGraphValidationException`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}", .0.join("\n"))]
pub struct ConceptGraphValidation(pub Vec<String>);

/// Origine di un'evidenza scritta prima che il campo esistesse: le evidenze scoperte da
/// kubirag hanno sempre id deterministico `ev-*`, quelle collegate a mano un UUID.
pub fn resolved_origin(evidence: &AuthorityConceptEvidence) -> AuthorityEvidenceOrigin {
    evidence
        .origin
        .unwrap_or(if evidence.evidence_id.starts_with("ev-") {
            AuthorityEvidenceOrigin::Promoted
        } else {
            AuthorityEvidenceOrigin::Manual
        })
}

/// Forma canonica scritta da kubirag: alias e notebook senza duplicati e ordinati, relazioni
/// ed evidenze deduplicate e ordinate, concetti ordinati per nome (senza maiuscole) e id.
pub fn canonical(graph: AuthorityConceptGraphFile) -> AuthorityConceptGraphFile {
    let mut graph = graph;
    for concept in &mut graph.concepts {
        canonicalize_concept(concept);
    }
    // `sortedWith` di Kotlin è stabile come `sort_by`.
    graph.concepts.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    graph
}

fn canonicalize_concept(concept: &mut GovernedConcept) {
    let mut seen = HashSet::new();
    concept
        .aliases
        .retain(|alias| seen.insert(alias.to_lowercase()));
    concept.aliases.sort_by_key(|alias| alias.to_lowercase());

    let notebooks: IndexSet<String> = concept.notebook_ids.drain(..).collect();
    concept.notebook_ids = notebooks.into_iter().collect();
    concept.notebook_ids.sort();

    let relations: IndexSet<_> = concept.relations.drain(..).collect();
    concept.relations = relations.into_iter().collect();
    concept.relations.sort_by(|a, b| {
        a.predicate
            .cmp(&b.predicate)
            .then_with(|| a.object_id.cmp(&b.object_id))
    });

    let mut seen = HashSet::new();
    let mut evidence: Vec<_> = concept
        .evidence
        .drain(..)
        .map(|mut e| {
            e.origin = Some(resolved_origin(&e));
            e
        })
        .filter(|e| seen.insert(e.evidence_id.clone()))
        .collect();
    evidence.sort_by(|a, b| a.evidence_id.cmp(&b.evidence_id));
    concept.evidence = evidence;
}

/// Invarianti del grafo. Versione, id e revisione dell'envelope sono già garantiti dal tipo.
pub fn validate(graph: &AuthorityConceptGraphFile) -> Result<(), ConceptGraphValidation> {
    let mut errors = Vec::new();
    let mut occurrences: HashMap<&str, usize> = HashMap::new();
    for concept in &graph.concepts {
        *occurrences.entry(concept.id.as_str()).or_default() += 1;
    }
    let mut duplicates: Vec<&str> = occurrences
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(id, _)| *id)
        .collect();
    duplicates.sort();
    errors.extend(
        duplicates
            .iter()
            .map(|id| format!("Duplicate concept id: {id}")),
    );

    let ids: HashSet<&str> = graph.concepts.iter().map(|c| c.id.as_str()).collect();
    for concept in &graph.concepts {
        if !is_jvm_uuid(&concept.id) {
            errors.push(format!("Invalid concept UUID '{}'", concept.id));
        }
        for notebook in concept.notebook_ids.iter().filter(|id| !is_jvm_uuid(id)) {
            errors.push(format!(
                "Invalid notebook UUID '{notebook}' on concept {}",
                concept.id
            ));
        }
        if concept.name.trim().is_empty() {
            errors.push("Blank concept name".to_owned());
        }
        for relation in concept
            .relations
            .iter()
            .filter(|r| r.object_ref.is_none() && !ids.contains(r.object_id.as_str()))
        {
            errors.push(format!(
                "Dangling legacy concept relation {} -> {}",
                concept.id, relation.object_id
            ));
        }
        for evidence in concept
            .evidence
            .iter()
            .filter(|e| e.concept_id != concept.id)
        {
            errors.push(format!(
                "Evidence {} has wrong conceptId",
                evidence.evidence_id
            ));
        }
    }
    let mut identities: HashMap<String, usize> = HashMap::new();
    for identity in graph
        .concepts
        .iter()
        .flat_map(|c| std::iter::once(&c.name).chain(c.aliases.iter()))
    {
        *identities.entry(identity.to_lowercase()).or_default() += 1;
    }
    errors.extend(
        identities
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .map(|(identity, _)| format!("Duplicate concept name or alias: {identity}")),
    );
    if errors.is_empty() {
        Ok(())
    } else {
        errors.sort();
        Err(ConceptGraphValidation(errors))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(json: &str) -> AuthorityConceptGraphFile {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn canonical_form_sorts_and_deduplicates() {
        let raw = graph(
            r#"{"id":"6e0de910-4426-48f0-afc5-b21dded49048","concepts":[
            {"id":"b","name":"beta","description":"","aliases":["Z","z","a"],
             "notebookIds":["2","1","2"],"createdAt":"t"},
            {"id":"a","name":"Alfa","description":"","createdAt":"t",
             "evidence":[{"conceptId":"a","sourceId":"s","evidenceId":"ev-2"},
                         {"conceptId":"a","sourceId":"s","evidenceId":"x"},
                         {"conceptId":"a","sourceId":"s","evidenceId":"ev-2"}]}]}"#,
        );
        let canonical = canonical(raw);
        let names: Vec<_> = canonical.concepts.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Alfa", "beta"]);
        assert_eq!(canonical.concepts[1].aliases, ["a", "Z"]);
        assert_eq!(canonical.concepts[1].notebook_ids, ["1", "2"]);
        let origins: Vec<_> = canonical.concepts[0]
            .evidence
            .iter()
            .map(|e| (e.evidence_id.as_str(), e.origin))
            .collect();
        assert_eq!(
            origins,
            [
                ("ev-2", Some(AuthorityEvidenceOrigin::Promoted)),
                ("x", Some(AuthorityEvidenceOrigin::Manual))
            ]
        );
    }

    #[test]
    fn validation_collects_every_problem() {
        let invalid = graph(
            r#"{"id":"6e0de910-4426-48f0-afc5-b21dded49048","concepts":[
            {"id":"nope","name":"A","description":"","aliases":["b"],"createdAt":"t",
             "relations":[{"predicate":"p","objectId":"missing"}]},
            {"id":"1-2-3-4-5","name":"B","description":"","createdAt":"t"}]}"#,
        );
        assert_eq!(
            validate(&invalid).unwrap_err().0,
            [
                "Dangling legacy concept relation nope -> missing",
                "Duplicate concept name or alias: b",
                "Invalid concept UUID 'nope'",
            ]
        );
    }
}
