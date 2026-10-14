import { ref } from 'vue';
import { governedFetch } from '/web/modules/governance.js';

export function usePinned(refreshSources) {
    const pinnedDocs = ref([]);
    const pinnedPanelOpen = ref(true);

    async function loadPinned() {
        try {
            const res = await fetch('/pinned');
            if (res.ok) pinnedDocs.value = await res.json();
        } catch (_) {}
    }

    function isPinned(docId) {
        return pinnedDocs.value.some(d => d.id === docId);
    }

    async function togglePin(doc) {
        const method = isPinned(doc.id) ? 'DELETE' : 'POST';
        try {
            const res = await governedFetch(`/pinned/${encodeURIComponent(doc.id)}`, { method });
            if (res.ok) await loadPinned();
        } catch (e) { console.warn('Pin toggle failed:', e.message); }
    }

    async function unpinDocument(docId) {
        try {
            const res = await governedFetch(`/pinned/${encodeURIComponent(docId)}`, { method: 'DELETE' });
            if (res.ok) await loadPinned();
        } catch (e) { console.warn('Unpin failed:', e.message); }
    }

    async function deleteSource(doc) {
        const label = doc.filename || doc.path || doc.id;
        if (!confirm(`Rimuovere la sorgente "${label}" dal contesto RAG?`)) return;
        try {
            const res = await governedFetch(`/documents/${encodeURIComponent(doc.id)}`, { method: 'DELETE' });
            if (res.ok) {
                await refreshSources();
                await loadPinned();
            } else {
                console.warn('Delete source failed:', res.status);
            }
        } catch (e) { console.warn('Delete source failed:', e.message); }
    }

    async function deletePdfGroup(group) {
        const label = group.dir || 'PDF';
        if (!confirm(`Rimuovere l'intero PDF "${label}" dalle sorgenti RAG? Il file originale rimarrà sul disco.`)) return;
        try {
            const res = await governedFetch(
                `/documents/groups/${encodeURIComponent(group.parentId)}`,
                { method: 'DELETE' }
            );
            if (!res.ok) {
                const error = await res.json().catch(() => ({}));
                alert(error.error || `PDF removal failed (HTTP ${res.status})`);
                return;
            }
            await refreshSources();
            await loadPinned();
        } catch (e) {
            alert('PDF removal failed: ' + e.message);
        }
    }

    function isFolderAllPinned(docs) {
        return docs.length > 0 && docs.every(d => isPinned(d.id));
    }

    function isFolderAnyPinned(docs) {
        return docs.some(d => isPinned(d.id));
    }

    async function togglePinFolder(docs) {
        if (docs.length === 0) return;
        const allPinned = isFolderAllPinned(docs);
        const method = allPinned ? 'DELETE' : 'POST';
        const toChange = docs.filter(d => allPinned ? isPinned(d.id) : !isPinned(d.id));
        for (const doc of toChange) {
            try {
                await governedFetch(`/pinned/${encodeURIComponent(doc.id)}`, { method });
            } catch (e) { console.warn('Folder pin failed:', e.message); }
        }
        await loadPinned();
    }

    return {
        pinnedDocs, pinnedPanelOpen, loadPinned, isPinned, togglePin,
        unpinDocument, deleteSource, deletePdfGroup,
        isFolderAllPinned, isFolderAnyPinned, togglePinFolder
    };
}
