import { nextTick, ref } from 'vue';

export function useMindmap() {
    const mindmapOpen = ref(false);
    const mindmapLoading = ref(false);
    const mindmapData = ref(null);
    const mindmapError = ref('');
    const mindmapContainer = ref(null);
    // Two-tab model: 'manual' (user-edited mindmap.md, priority) vs 'auto' (generated).
    const mindmapTab = ref('auto');
    const mindmapManualExists = ref(false);
    const mindmapEditing = ref(false);
    const mindmapSource = ref('');
    const mindmapSaving = ref(false);
    const mindmapManualContainer = ref(null);

    async function renderMermaidInto(container, mermaidStr) {
        if (!container || !mermaidStr) return;
        const { svg } = await mermaid.render('mindmap-diagram-' + Date.now(), mermaidStr);
        container.innerHTML = svg;
        const svgEl = container.querySelector('svg');
        if (svgEl) {
            svgEl.style.width = '100%';
            svgEl.style.height = '100%';
            svgEl.removeAttribute('width');
            svgEl.removeAttribute('height');
        }
    }

    // Renders whichever tab is active. 'auto' always asks the server for the generated map
    // (source=auto), 'manual' renders the saved mindmap.md (empty state handled by the template).
    async function renderActiveTab() {
        mindmapError.value = '';
        if (mindmapTab.value === 'manual') {
            if (!mindmapManualExists.value) return;
            try {
                const res = await fetch('/mindmap/manual');
                if (!res.ok) throw new Error(`HTTP ${res.status}`);
                const data = await res.json();
                mindmapManualExists.value = data.exists;
                await nextTick();
                await renderMermaidInto(mindmapManualContainer.value, data.mermaid);
            } catch (e) {
                mindmapError.value = 'Manual map error: ' + e.message;
            }
            return;
        }
        mindmapLoading.value = true;
        mindmapData.value = null;
        try {
            const res = await fetch('/mindmap?source=auto');
            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            const data = await res.json();
            mindmapData.value = data;
            mindmapManualExists.value = data.manualExists || mindmapManualExists.value;
            mindmapLoading.value = false;
            await nextTick();
            await renderMermaidInto(mindmapContainer.value, data.mermaid);
        } catch (e) {
            mindmapError.value = 'Map generation error: ' + e.message;
            mindmapLoading.value = false;
        }
    }

    async function openMindmap() {
        mindmapOpen.value = true;
        mindmapEditing.value = false;
        mindmapError.value = '';
        // Default tab = manual when mindmap.md exists (priority), else auto.
        try {
            const res = await fetch('/mindmap/manual');
            const data = res.ok ? await res.json() : { exists: false };
            mindmapManualExists.value = !!data.exists;
        } catch (e) {
            mindmapManualExists.value = false;
        }
        mindmapTab.value = mindmapManualExists.value ? 'manual' : 'auto';
        await renderActiveTab();
    }

    async function selectMindmapTab(tab) {
        mindmapTab.value = tab;
        mindmapEditing.value = false;
        await renderActiveTab();
    }

    // Enter edit mode: load the existing note, or seed a fenced mermaid block from the auto map.
    async function editManualMindmap() {
        mindmapError.value = '';
        mindmapTab.value = 'manual';
        try {
            const res = await fetch('/mindmap/manual');
            const data = res.ok ? await res.json() : { exists: false, content: '' };
            if (data.exists && data.content) {
                mindmapSource.value = data.content;
            } else {
                const autoRes = await fetch('/mindmap?source=auto');
                const auto = autoRes.ok ? await autoRes.json() : { mermaid: 'mindmap\n  root((KubiRAG))' };
                mindmapSource.value = '```mermaid\n' + (auto.mermaid || 'mindmap\n  root((KubiRAG))') + '\n```\n';
            }
            mindmapEditing.value = true;
        } catch (e) {
            mindmapError.value = 'Manual map load error: ' + e.message;
        }
    }

    // Client-side preview of the textarea content without persisting.
    async function previewManualMindmap() {
        mindmapError.value = '';
        const mermaidStr = extractMermaidClient(mindmapSource.value);
        if (!mermaidStr) { mindmapError.value = 'No mermaid diagram found in the note.'; return; }
        await nextTick();
        try {
            await renderMermaidInto(mindmapManualContainer.value, mermaidStr);
        } catch (e) {
            mindmapError.value = 'Preview error: ' + e.message;
        }
    }

    async function saveManualMindmap() {
        mindmapSaving.value = true;
        mindmapError.value = '';
        try {
            const res = await fetch('/mindmap/manual', {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ content: mindmapSource.value })
            });
            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            mindmapEditing.value = false;
            mindmapManualExists.value = true;
            mindmapTab.value = 'manual';
            await renderActiveTab();
        } catch (e) {
            mindmapError.value = 'Save error: ' + e.message;
        } finally {
            mindmapSaving.value = false;
        }
    }

    async function deleteManualMindmap() {
        mindmapError.value = '';
        try {
            const res = await fetch('/mindmap/manual', { method: 'DELETE' });
            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            mindmapManualExists.value = false;
            mindmapEditing.value = false;
            mindmapTab.value = 'auto';
            await renderActiveTab();
        } catch (e) {
            mindmapError.value = 'Delete error: ' + e.message;
        }
    }

    // Mirrors CtrlMindmap.extractMermaid for client-side preview.
    function extractMermaidClient(markdown) {
        const fence = /```mermaid\s*\n([\s\S]*?)```/.exec(markdown || '');
        if (fence) return fence[1].trim();
        const trimmed = (markdown || '').trim();
        const firstWord = trimmed.split('\n')[0].trim().split(' ')[0].toLowerCase();
        const keywords = ['mindmap', 'graph', 'flowchart', 'erdiagram', 'sequencediagram',
            'classdiagram', 'statediagram', 'statediagram-v2', 'gantt', 'journey',
            'pie', 'gitgraph', 'timeline', 'quadrantchart', 'mermaid'];
        return keywords.includes(firstWord) ? trimmed : '';
    }

    return {
        mindmapOpen, mindmapLoading, mindmapData, mindmapError, mindmapContainer, openMindmap,
        mindmapTab, mindmapManualExists, mindmapEditing, mindmapSource, mindmapSaving, mindmapManualContainer,
        selectMindmapTab, editManualMindmap, previewManualMindmap, saveManualMindmap, deleteManualMindmap
    };
}

export function useWiki() {
    const wikiGenerating = ref(false);
    const wikiModalOpen = ref(false);
    const wikiContentSections = ref(['sources']);
    const wikiOutputDir = ref('wiki');
    const wikiOverwrite = ref(false);
    const wikiPageIds = ref(null);
    const wikiPageIdsBusy = ref(false);

    async function refreshWikiPageIds() {
        const res = await fetch('/wiki/page-ids/status');
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        wikiPageIds.value = await res.json();
    }

    async function prepareWikiPageIds() {
        wikiPageIdsBusy.value = true;
        try {
            if (!window.confirm('Assign a stable UUID to every legacy note that does not have one?')) return;
            const res = await fetch('/wiki/page-ids/prepare', { method: 'POST' });
            if (!res.ok) {
                const err = await res.json().catch(() => ({}));
                throw new Error(err.error || `HTTP ${res.status}`);
            }
            const data = await res.json();
            wikiPageIds.value = data.status;
        } catch (e) {
            alert('Page ID preparation error: ' + e.message);
        } finally {
            wikiPageIdsBusy.value = false;
        }
    }

    function useWikiPublicationPreset() {
        wikiContentSections.value = ['notes', 'concepts'];
        wikiOutputDir.value = 'src/site/resources/wiki';
        wikiOverwrite.value = true;
    }

    function openWikiModal() {
        wikiOverwrite.value = false;
        wikiModalOpen.value = true;
        refreshWikiPageIds().catch(e => { wikiPageIds.value = { error: e.message }; });
    }

    async function generateWiki() {
        wikiGenerating.value = true;
        try {
            const res = await fetch('/wiki/generate', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    content: wikiContentSections.value.join(','),
                    outputDir: wikiOutputDir.value.trim() || 'wiki',
                    overwrite: wikiOverwrite.value
                })
            });
            if (!res.ok) {
                const err = await res.json().catch(() => ({}));
                throw new Error(err.error || `HTTP ${res.status}`);
            }
            wikiModalOpen.value = false;
            window.open('/wiki/', '_blank');
        } catch (e) {
            alert('Wiki generation error: ' + e.message);
        } finally {
            wikiGenerating.value = false;
        }
    }

    return {
        wikiGenerating, wikiModalOpen, wikiContentSections, wikiOutputDir, wikiOverwrite,
        wikiPageIds, wikiPageIdsBusy,
        openWikiModal, generateWiki, refreshWikiPageIds, prepareWikiPageIds, useWikiPublicationPreset
    };
}

export function useSlides() {
    const slideModalOpen = ref(false);
    const slideGenerating = ref(false);
    const slideOutputDir = ref('slides');

    function openSlideModal() {
        slideModalOpen.value = true;
    }

    async function generateSlides() {
        slideGenerating.value = true;
        try {
            const res = await fetch('/slides/generate', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ outputDir: slideOutputDir.value.trim() || 'slides' })
            });
            if (!res.ok) {
                const error = await res.json().catch(() => ({}));
                throw new Error(error.error || `HTTP ${res.status}`);
            }
            const data = await res.json();
            slideModalOpen.value = false;
            window.open(data.url || `/slides/${encodeURIComponent(data.filename)}`, '_blank');
        } catch (e) {
            alert('Slide generation error: ' + e.message);
        } finally {
            slideGenerating.value = false;
        }
    }

    return { slideModalOpen, slideGenerating, slideOutputDir, openSlideModal, generateSlides };
}
