use kubi_kernel::governance::{
    ModelIssue, ModelIssueSeverity, OperationContext, OperationResult, OperationStatus,
    RevisionConflictError,
};
use kubirag_domain::notebook::{
    NotebookError, RagNotebookAuditEvent, RagNotebookAuditSnapshot, RagNotebookCommand,
    RagNotebookCommandType, RagNotebookManifest, ResourceFingerprints,
};

use super::{
    Clock, IdGenerator, ManifestWriteError, NotebookAuditLog, NotebookProjection,
    RagNotebookManifestManager,
};

const LABEL: &str = "RAG Notebook";

/// Servizio applicativo del notebook (`RagNotebookApplicationService`).
///
/// Ogni comando diventa una transizione governata: controllo della revisione attesa,
/// decisione del dominio, commit con compare-and-set, ricostruzione delle proiezioni e un
/// domain event nel journal, anche per i comandi rifiutati o in conflitto. Il risultato è un
/// [`OperationResult`] con gli stessi stati e messaggi della versione Kotlin.
pub struct RagNotebookApplicationService {
    manager: RagNotebookManifestManager,
    audit: Box<dyn NotebookAuditLog>,
    projection: Box<dyn NotebookProjection>,
    fingerprints: Box<dyn ResourceFingerprints>,
    clock: Box<dyn Clock>,
    ids: Box<dyn IdGenerator>,
    visible_issues: Vec<ModelIssue>,
}

enum Failure {
    Conflict(String),
    Rejected(String),
}

impl From<NotebookError> for Failure {
    fn from(error: NotebookError) -> Self {
        Self::Rejected(error.to_string())
    }
}

impl From<ManifestWriteError> for Failure {
    fn from(error: ManifestWriteError) -> Self {
        match error {
            ManifestWriteError::RevisionConflict(conflict) => Self::Conflict(conflict.to_string()),
            other => Self::Rejected(other.to_string()),
        }
    }
}

impl RagNotebookApplicationService {
    pub fn new(
        manager: RagNotebookManifestManager,
        audit: Box<dyn NotebookAuditLog>,
        projection: Box<dyn NotebookProjection>,
        fingerprints: Box<dyn ResourceFingerprints>,
        clock: Box<dyn Clock>,
        ids: Box<dyn IdGenerator>,
    ) -> Self {
        let visible_issues = manager.issues();
        Self {
            manager,
            audit,
            projection,
            fingerprints,
            clock,
            ids,
            visible_issues,
        }
    }

    pub fn snapshot(&self) -> &RagNotebookManifest {
        self.manager.snapshot()
    }

    pub fn issues(&self) -> &[ModelIssue] {
        &self.visible_issues
    }

    pub fn audit_events(&self) -> Result<Vec<RagNotebookAuditEvent>, String> {
        self.audit.events()
    }

    /// Allinea il servizio al manifest su disco (`withActiveService` in Kotlin ricarica il
    /// manifest a ogni richiesta). Se è cambiato, le proiezioni sono ricostruite senza
    /// produrre un evento: il commit e il suo evento appartengono a chi l'ha scritto.
    pub fn refresh(&mut self) -> Result<bool, String> {
        let before = self.manager.snapshot().clone();
        if !self.manager.reload().map_err(|e| e.to_string())? {
            return Ok(false);
        }
        let after = self.manager.snapshot().clone();
        let failure = self.projection.rebuild(&before, &after).err();
        self.visible_issues = self.manager.issues();
        self.visible_issues
            .extend(failure.as_ref().map(|f| projection_issue(&after, f)));
        Ok(true)
    }

    /// Esegue un comando governato.
    pub fn execute(
        &mut self,
        command: &RagNotebookCommand,
        context: &OperationContext,
    ) -> OperationResult {
        let event_id = self.ids.new_id();
        let before = self.manager.snapshot().clone();
        let command_type = command.command_type();
        let target_id = command.target_id(&before);
        match self.commit(command, context, &before) {
            Ok(committed) => {
                let failure = self.projection.rebuild(&before, &committed).err();
                self.visible_issues = self.manager.issues();
                self.visible_issues
                    .extend(failure.as_ref().map(|f| projection_issue(&committed, f)));
                let message = match failure {
                    None => format!(
                        "{command_type} applied at revision {}",
                        committed.revision()
                    ),
                    Some(_) => format!(
                        "{command_type} committed at revision {}, but projection failed",
                        committed.revision()
                    ),
                };
                let event = self.event(
                    event_id,
                    command_type,
                    target_id,
                    context,
                    OperationStatus::Applied,
                    None,
                    &before,
                    &committed,
                );
                self.applied(event, message, context, &before, &committed)
            }
            Err(Failure::Conflict(diagnostic)) => self.recorded(
                event_id,
                command_type,
                target_id,
                context,
                OperationStatus::Conflicted,
                diagnostic,
                &before,
            ),
            Err(Failure::Rejected(diagnostic)) => self.recorded(
                event_id,
                command_type,
                target_id,
                context,
                OperationStatus::Rejected,
                diagnostic,
                &before,
            ),
        }
    }

    /// Ricostruisce le viste derivate senza creare una nuova revisione del modello.
    pub fn rebuild_projections(&mut self, context: &OperationContext) -> OperationResult {
        let snapshot = self.manager.snapshot().clone();
        let command = RagNotebookCommandType::RebuildProjections;
        let target = snapshot.name().to_owned();
        let event_id = self.ids.new_id();
        if snapshot.revision() != context.expected_revision() {
            let diagnostic = conflict(context.expected_revision(), snapshot.revision());
            return self.recorded(
                event_id,
                command,
                target,
                context,
                OperationStatus::Conflicted,
                diagnostic,
                &snapshot,
            );
        }
        let failure = self.projection.rebuild(&snapshot, &snapshot).err();
        self.visible_issues = self.manager.issues();
        self.visible_issues
            .extend(failure.as_ref().map(|f| projection_issue(&snapshot, f)));
        match failure {
            None => {
                let message = format!(
                    "Notebook projections rebuilt at revision {}",
                    snapshot.revision()
                );
                let event = self.event(
                    event_id,
                    command,
                    target,
                    context,
                    OperationStatus::Applied,
                    None,
                    &snapshot,
                    &snapshot,
                );
                self.applied(event, message, context, &snapshot, &snapshot)
            }
            Some(failure) => self.recorded(
                event_id,
                command,
                target,
                context,
                OperationStatus::Rejected,
                format!("Notebook projection rebuild failed: {failure}"),
                &snapshot,
            ),
        }
    }

    /// L'undo non esiste: si usa un comando compensativo esplicito. Il rifiuto è registrato.
    pub fn undo_unavailable(&mut self, context: &OperationContext) -> OperationResult {
        let snapshot = self.manager.snapshot().clone();
        let event_id = self.ids.new_id();
        self.recorded(
            event_id,
            RagNotebookCommandType::Undo,
            snapshot.name().to_owned(),
            context,
            OperationStatus::Rejected,
            "Undo is unavailable for RAG Notebook; use an explicit compensating command".into(),
            &snapshot,
        )
    }

    /// Registra una precondizione scaduta prima che un adattatore faccia I/O preparatorio.
    pub fn preflight(
        &mut self,
        command: RagNotebookCommandType,
        target_id: &str,
        context: &OperationContext,
    ) -> Option<OperationResult> {
        let snapshot = self.manager.snapshot().clone();
        if snapshot.revision() == context.expected_revision() {
            return None;
        }
        let event_id = self.ids.new_id();
        Some(self.recorded(
            event_id,
            command,
            target_id.to_owned(),
            context,
            OperationStatus::Conflicted,
            conflict(context.expected_revision(), snapshot.revision()),
            &snapshot,
        ))
    }

    fn commit(
        &mut self,
        command: &RagNotebookCommand,
        context: &OperationContext,
        before: &RagNotebookManifest,
    ) -> Result<RagNotebookManifest, Failure> {
        if before.revision() != context.expected_revision() {
            return Err(Failure::Conflict(conflict(
                context.expected_revision(),
                before.revision(),
            )));
        }
        let next = command.decide(before, self.fingerprints.as_ref())?;
        Ok(self
            .manager
            .commit(&next, context.expected_revision())?
            .clone())
    }

    fn applied(
        &self,
        event: RagNotebookAuditEvent,
        message: String,
        context: &OperationContext,
        before: &RagNotebookManifest,
        after: &RagNotebookManifest,
    ) -> OperationResult {
        match self.audit.append(event.clone()) {
            Ok(()) => result(&event, message),
            Err(failure) => OperationResult::new(
                OperationStatus::AuditFailed,
                format!("{message}; audit failed: {failure}"),
                context.correlation_id(),
                context.expected_revision(),
            )
            .and_then(|r| r.with_revisions(before.revision(), after.revision()))
            .map(|r| r.with_issues(self.visible_issues.clone()))
            .expect("revisions of committed manifests are not negative"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn recorded(
        &self,
        id: String,
        command: RagNotebookCommandType,
        target_id: String,
        context: &OperationContext,
        status: OperationStatus,
        diagnostic: String,
        snapshot: &RagNotebookManifest,
    ) -> OperationResult {
        let event = self.event(
            id,
            command,
            target_id,
            context,
            status,
            Some(diagnostic.clone()),
            snapshot,
            snapshot,
        );
        // Kotlin: `runCatching { audit.append(event) }`, il rifiuto resta il risultato.
        let _ = self.audit.append(event.clone());
        result(&event, diagnostic)
    }

    #[allow(clippy::too_many_arguments)]
    fn event(
        &self,
        id: String,
        command: RagNotebookCommandType,
        target_id: String,
        context: &OperationContext,
        status: OperationStatus,
        diagnostic: Option<String>,
        before: &RagNotebookManifest,
        after: &RagNotebookManifest,
    ) -> RagNotebookAuditEvent {
        RagNotebookAuditEvent {
            id,
            command,
            target_id,
            actor: context.actor().to_owned(),
            channel: context.channel(),
            correlation_id: context.correlation_id().to_owned(),
            timestamp: self.clock.now(),
            expected_revision: context.expected_revision(),
            previous_revision: before.revision(),
            current_revision: after.revision(),
            status,
            diagnostic,
            issues: self.visible_issues.clone(),
            before: RagNotebookAuditSnapshot::of(before),
            after: RagNotebookAuditSnapshot::of(after),
        }
    }
}

fn conflict(expected: i64, actual: i64) -> String {
    RevisionConflictError {
        label: LABEL,
        expected,
        actual: Some(actual),
    }
    .to_string()
}

fn projection_issue(manifest: &RagNotebookManifest, failure: &str) -> ModelIssue {
    let message = if failure.trim().is_empty() {
        "Notebook projection failed"
    } else {
        failure
    };
    ModelIssue::new(
        "RAG_NOTEBOOK_PROJECTION_FAILED",
        ModelIssueSeverity::Error,
        format!("notebook/{}/projections", manifest.id()),
        message,
        true,
    )
    .expect("code, path and message are not blank")
}

fn result(event: &RagNotebookAuditEvent, message: String) -> OperationResult {
    let message = if message.trim().is_empty() {
        "Notebook command rejected".to_owned()
    } else {
        message
    };
    OperationResult::new(
        event.status,
        message,
        event.correlation_id.clone(),
        event.expected_revision,
    )
    .and_then(|r| r.with_revisions(event.previous_revision, event.current_revision))
    .map(|r| {
        r.with_issues(event.issues.clone())
            .with_audit_event(event.id.clone())
    })
    .expect("event revisions are not negative")
}
