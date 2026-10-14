        import { createApp, ref, computed, watch, nextTick, onMounted, onUnmounted, provide } from 'vue';
        import {
            AppHeader,
            SourcesPanel,
            WorkspacePanel,
            StudioPanel,
            VueModals
        } from '/web/components.js';
        import { installDiagramPopup, viewerDiagramSources } from '/web/modules/diagram-popup.js';
        import { renderConceptMap } from '/web/modules/concept-map.js';
        import { governedFetch } from '/web/modules/governance.js';
        import { useSettings } from '/web/composables/settings.js';
        import { usePinned } from '/web/composables/pinned.js';
        import { useUploads } from '/web/composables/uploads.js';
        import { useMindmap, useSlides, useWiki } from '/web/composables/studio.js';

        installDiagramPopup();

        createApp({
            components: {
                AppHeader,
                SourcesPanel,
                WorkspacePanel,
                StudioPanel,
                VueModals
            },
            template: `
                <AppHeader />
                <div class="container">
                    <SourcesPanel />
                    <WorkspacePanel />
                    <StudioPanel />
                </div>
                <VueModals />
            `,
            setup() {
                // --- Chat state ---
                const messages = ref([]);
                const userInput = ref('');
                const isStreaming = ref(false);
                const sessionId = ref('default');
                const chatWithoutRetrieval = ref(false);
                const chatContent = ref(null);
                const inputArea = ref(null);
                const openChatTurn = ref(null);

                // Keep the transport/session message format untouched and derive accordion turns
                // solely for presentation. A user message starts a turn; following non-user
                // messages belong to it. Imported histories that start with an answer still render.
                const chatTurns = computed(() => {
                    const turns = [];
                    let current = null;
                    messages.value.forEach((message, index) => {
                        if (message.role === 'user') {
                            current = {
                                key: `chat-turn-${index}`,
                                question: message.content || 'Question',
                                answers: []
                            };
                            turns.push(current);
                        } else {
                            if (!current) {
                                current = {
                                    key: `chat-turn-${index}`,
                                    question: 'KubiRAG response',
                                    answers: []
                                };
                                turns.push(current);
                            }
                            current.answers.push({ index, message });
                        }
                    });
                    return turns;
                });

                // Bootstrap-like exclusive accordion: selecting another item closes the current
                // one, while selecting the already-open item deliberately leaves it open.
                function openChatTurnItem(key) {
                    if (openChatTurn.value !== key) openChatTurn.value = key;
                }

                function openLatestChatTurn() {
                    const latest = chatTurns.value[chatTurns.value.length - 1];
                    openChatTurn.value = latest ? latest.key : null;
                }

                // --- Sources state ---
                const leftTab = ref('notes');
                const sources = ref([]);
                const sourcesLoading = ref(true);
                const sourceFilter = ref('');
                const luceneResults = ref([]);
                const luceneSearchActive = ref(false);

                // --- Doc viewer state ---
                const viewerOpen = ref(false);
                const viewerLoading = ref(false);
                const viewerDoc = ref({ filename: '', path: '' });
                const viewerHtml = ref('');
                const viewerEnrichment = ref(null);

                const {
                    settingsOpen, settingsLoading, settingsSaved, settings,
                    contextUsage, availableModels, modelsLoading,
                    loadContextUsage, loadModels, openSettingsPanel, saveSettings
                } = useSettings(sessionId);

                const {
                    uploading, uploadError, uploadSuccess, draggingOver, fileInputRef,
                    pdfModal, addSourceModal, confirmPdfChunked, scrapeDocument, uploadDocument,
                    handleFileSelect, handleDrop, handleDragOver, handleDragLeave
                } = useUploads({ refreshSources, loadTermIndex });

                const {
                    pinnedDocs, pinnedPanelOpen, loadPinned, isPinned, togglePin,
                    unpinDocument, deleteSource, deletePdfGroup,
                    isFolderAllPinned, isFolderAnyPinned, togglePinFolder
                } = usePinned(refreshSources);

                const {
                    mindmapOpen, mindmapLoading, mindmapData,
                    mindmapError, mindmapContainer, openMindmap,
                    mindmapTab, mindmapManualExists, mindmapEditing, mindmapSource, mindmapSaving, mindmapManualContainer,
                    selectMindmapTab, editManualMindmap, previewManualMindmap, saveManualMindmap, deleteManualMindmap
                } = useMindmap();
                const {
                    wikiGenerating, wikiModalOpen, wikiContentSections,
                    wikiOutputDir, wikiOverwrite, wikiPageIds, wikiPageIdsBusy,
                    openWikiModal, generateWiki, refreshWikiPageIds, prepareWikiPageIds, useWikiPublicationPreset
                } = useWiki();

                // --- Notebook properties state (per-store: author, cover, wiki background, ...) ---
                const notebookModalOpen = ref(false);
                const notebookSaving = ref(false);
                const notebookStoreName = ref('');
                const notebookDefaultTitle = ref('');
                // The store whose properties the modal is editing. Blank = active store (used by the
                // book-modal pre-fill); set to a specific name when opened from the Notebooks list.
                const notebookEditStore = ref('');
                const notebookProps = ref({
                    author: '', title: '', subtitle: '', description: '',
                    coverImage: '', backgroundImage: '', accentColor: ''
                });
                // Appends the edited store as a query param so requests target it without loading it.
                function notebookStoreQuery(prefixChar) {
                    const s = notebookEditStore.value.trim();
                    return s ? prefixChar + 'store=' + encodeURIComponent(s) : '';
                }
                // Preview only what the browser can actually load: external URLs and imported assets.
                function notebookPreview(value) {
                    const v = (value || '').trim();
                    if (/^https?:\/\//i.test(v)) return v;
                    if (v.startsWith('assets/')) return '/notebook/asset/' + encodeURIComponent(v.slice('assets/'.length)) + notebookStoreQuery('?');
                    return '';
                }
                async function loadNotebookProps(store) {
                    const q = store ? '?store=' + encodeURIComponent(store) : '';
                    const res = await fetch('/notebook' + q);
                    if (!res.ok) throw new Error(`HTTP ${res.status}`);
                    const d = await res.json();
                    notebookStoreName.value = d.storeName || '';
                    notebookDefaultTitle.value = d.defaultTitle || '';
                    notebookProps.value = {
                        author: d.author || '', title: d.title || '', subtitle: d.subtitle || '',
                        description: d.description || '', coverImage: d.coverImage || '',
                        backgroundImage: d.backgroundImage || '', accentColor: d.accentColor || ''
                    };
                    return d;
                }
                async function openNotebookModal(store) {
                    // store may be a string (from the Notebooks list) or undefined/event (active store).
                    notebookEditStore.value = (typeof store === 'string') ? store : '';
                    try { await loadNotebookProps(notebookEditStore.value); } catch (e) { /* start from blanks on error */ }
                    notebookModalOpen.value = true;
                }
                async function saveNotebookProperties() {
                    notebookSaving.value = true;
                    try {
                        const notebookUrl = '/notebook' + notebookStoreQuery('?');
                        const res = await governedFetch(notebookUrl, {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify(notebookProps.value)
                        }, notebookUrl);
                        if (!res.ok) {
                            const err = await res.json().catch(() => ({}));
                            throw new Error(err.error || `HTTP ${res.status}`);
                        }
                        // Server returns managed refs (local paths rewritten to assets/<file>).
                        notebookProps.value = await res.json();
                        notebookModalOpen.value = false;
                    } catch (e) {
                        alert('Notebook properties error: ' + e.message);
                    } finally {
                        notebookSaving.value = false;
                    }
                }

                // --- Book state ---
                const bookGenerating = ref(false);
                const bookModalOpen = ref(false);
                const bookTitle = ref('KubiRAG Book');
                const bookEyebrow = ref('');
                const bookSubtitle = ref('Generated from notes');
                const bookAuthor = ref('');
                const bookCoverImage = ref('');
                const bookIncludeCover = ref(true);
                const bookIncludeToc = ref(true);
                const bookScope = ref('all');
                const bookOutputDir = ref('books');
                async function openBookModal() {
                    // Pre-fill cover defaults from the notebook's properties (without overwriting a
                    // value the user already typed in a prior open).
                    try {
                        const d = await loadNotebookProps();
                        if (d.author && !bookAuthor.value) bookAuthor.value = d.author;
                        if (d.coverImage && !bookCoverImage.value) bookCoverImage.value = d.coverImage;
                        if (d.title && bookTitle.value === 'KubiRAG Book') bookTitle.value = d.title;
                        if (d.subtitle && bookSubtitle.value === 'Generated from notes') bookSubtitle.value = d.subtitle;
                    } catch (e) { /* notebook props optional */ }
                    bookModalOpen.value = true;
                }
                // KaTeX delimiters kept identical to the in-app viewer (see postRenderViewer).
                const BOOK_KATEX_DELIMS = [
                    { left: '$$', right: '$$', display: true },
                    { left: '$',  right: '$',  display: false },
                    { left: '\\(', right: '\\)', display: false },
                    { left: '\\[', right: '\\]', display: true }
                ];

                async function bookFontsReady() {
                    try { if (document.fonts && document.fonts.ready) await document.fonts.ready; } catch (e) {}
                }

                // An off-screen, white, attached container (html-to-image needs real layout to measure).
                function bookOffscreen(widthPx) {
                    const el = document.createElement('div');
                    el.style.position = 'fixed';
                    el.style.left = '-100000px';
                    el.style.top = '0';
                    el.style.background = '#ffffff';
                    el.style.color = '#111';
                    el.style.padding = '2px 4px';
                    if (widthPx) { el.style.width = widthPx + 'px'; el.style.display = 'block'; }
                    else { el.style.display = 'inline-block'; }
                    return el;
                }

                async function bookRasterize(el) {
                    document.body.appendChild(el);
                    try {
                        await bookFontsReady();
                        return await window.htmlToImage.toPng(el, { pixelRatio: 2, backgroundColor: '#ffffff' });
                    } finally {
                        el.remove();
                    }
                }

                // Render a Mermaid diagram to a PNG data URL (null on failure → caller keeps source).
                let _bookCapSeq = 0;
                async function bookCaptureMermaid(src) {
                    try {
                        const { svg } = await mermaid.render('book-mmd-' + (_bookCapSeq++), src);
                        const el = bookOffscreen(null);
                        el.innerHTML = svg;
                        return await bookRasterize(el);
                    } catch (e) { console.warn('Book: mermaid capture failed', e); return null; }
                }

                // Render a display formula ($$…$$ / \[…\]) to a PNG data URL.
                async function bookCaptureDisplayMath(tex) {
                    try {
                        if (!window.katex) return null;
                        const el = bookOffscreen(null);
                        el.innerHTML = katex.renderToString(tex, { displayMode: true, throwOnError: false });
                        return await bookRasterize(el);
                    } catch (e) { console.warn('Book: display-math capture failed', e); return null; }
                }

                // Renders a whole markdown fragment (paragraph / list / heading / quote) that contains
                // inline math to a PNG, but ONLY if KaTeX actually produced math — guards against
                // currency like "$5 and $10". Returns null otherwise so the caller keeps source text.
                async function bookCaptureMarkdownFragment(md) {
                    try {
                        if (!window.renderMathInElement || !window.marked) return null;
                        const el = bookOffscreen(640);
                        el.innerHTML = marked.parse(md);
                        document.body.appendChild(el);
                        try {
                            renderMathInElement(el, { delimiters: BOOK_KATEX_DELIMS, throwOnError: false });
                            if (!el.querySelector('.katex')) return null; // no real math — keep source
                            await bookFontsReady();
                            return await window.htmlToImage.toPng(el, { pixelRatio: 2, backgroundColor: '#ffffff' });
                        } finally { el.remove(); }
                    } catch (e) { console.warn('Book: fragment capture failed', e); return null; }
                }

                // True when a line carries math KaTeX would render: inline $…$, \( … \) or any $$ / \[.
                function bookLineHasMath(l) {
                    return /(^|[^\\])\$[^$\n]*[^\s$][^$\n]*\$/.test(l) || /\\\(/.test(l) || /\$\$/.test(l) || /\\\[/.test(l);
                }

                // Rewrites a note's markdown so every block that contains math (Mermaid / display math /
                // or a paragraph·list·heading·quote with inline math) becomes a `![](capture:<id>)` ref,
                // returning the rewritten body + the backing PNGs. Blocks without math pass through as-is
                // (kept selectable); code fences are never touched. A capture that fails keeps its source.
                async function bookBuildOverlay(content) {
                    const lines = (content || '').replace(/\r\n/g, '\n').split('\n');
                    const out = [];
                    const captures = {};
                    let n = 0;
                    const emit = (url) => { const id = 'cap_' + (n++); captures[id] = url; out.push('![](capture:' + id + ')'); };
                    const isListLine = (l) => /^\s*([-*+]\s|\d+\.\s)/.test(l);
                    let i = 0;
                    while (i < lines.length) {
                        const line = lines[i];
                        const start = i;

                        // 1. Fenced code — mermaid is captured, everything else passes through verbatim.
                        const fence = line.match(/^\s*```(\w*)\s*$/);
                        if (fence) {
                            const body = [];
                            i++;
                            while (i < lines.length && !/^\s*```\s*$/.test(lines[i])) { body.push(lines[i]); i++; }
                            i++; // consume closing fence
                            if ((fence[1] || '').toLowerCase() === 'mermaid') {
                                const url = await bookCaptureMermaid(body.join('\n'));
                                if (url) emit(url); else { out.push('```mermaid'); out.push(...body); out.push('```'); }
                            } else { out.push('```' + (fence[1] || '')); out.push(...body); out.push('```'); }
                            continue;
                        }

                        // 2. Display math block: $$ … $$  |  \[ … \]  |  a lone $ … lone $ (multi-line LaTeX).
                        let disp = null;
                        if (/^\s*\$\$/.test(line)) disp = { open: /^\s*\$\$/, close: '$$' };
                        else if (/^\s*\\\[/.test(line)) disp = { open: /^\s*\\\[/, close: '\\]' };
                        else if (/^\s*\$\s*$/.test(line)) disp = { lone: true, close: '$' };
                        if (disp) {
                            let tex;
                            if (disp.lone) {
                                tex = ''; i++;
                                while (i < lines.length && !/^\s*\$\s*$/.test(lines[i])) { tex += lines[i] + '\n'; i++; }
                                i++; // closing lone $
                            } else {
                                const rest = line.replace(disp.open, '');
                                const idx = rest.indexOf(disp.close);
                                if (idx >= 0) { tex = rest.substring(0, idx); i++; }
                                else {
                                    tex = rest + '\n'; i++;
                                    while (i < lines.length && lines[i].indexOf(disp.close) < 0) { tex += lines[i] + '\n'; i++; }
                                    if (i < lines.length) { tex += lines[i].substring(0, lines[i].indexOf(disp.close)); i++; }
                                }
                            }
                            const url = await bookCaptureDisplayMath(tex.trim());
                            if (url) emit(url); else out.push(...lines.slice(start, i));
                            continue;
                        }

                        // 3. List block (one bulleted/numbered list, incl. blank separators) — captured
                        //    whole when any item has inline math, so list formatting survives.
                        if (isListLine(line)) {
                            i++;
                            while (i < lines.length && (isListLine(lines[i]) || lines[i].trim() === '' || /^\s+\S/.test(lines[i]))) i++;
                            let run = lines.slice(start, i);
                            const trail = [];
                            while (run.length && run[run.length - 1].trim() === '') { trail.unshift(run.pop()); }
                            if (run.some(bookLineHasMath)) {
                                const url = await bookCaptureMarkdownFragment(run.join('\n'));
                                if (url) emit(url); else out.push(...run);
                            } else out.push(...run);
                            out.push(...trail);
                            continue;
                        }

                        // 4. Heading / blockquote / horizontal rule / blank — pass through, but capture a
                        //    heading or quote that carries inline math.
                        if (/^\s*#{1,6}\s/.test(line)) {
                            if (bookLineHasMath(line)) { const url = await bookCaptureMarkdownFragment(line); if (url) { emit(url); i++; continue; } }
                            out.push(line); i++; continue;
                        }
                        if (/^\s*>/.test(line)) {
                            i++;
                            while (i < lines.length && /^\s*>/.test(lines[i])) i++;
                            const run = lines.slice(start, i);
                            if (run.some(bookLineHasMath)) { const url = await bookCaptureMarkdownFragment(run.join('\n')); if (url) emit(url); else out.push(...run); }
                            else out.push(...run);
                            continue;
                        }
                        if (line.trim() === '' || /^\s*([-*_])\1{2,}\s*$/.test(line) || /^\s*!\[/.test(line)) { out.push(line); i++; continue; }

                        // 5. Paragraph — gather the run, capture whole if it carries real inline math.
                        i++;
                        while (i < lines.length && lines[i].trim() !== '' && !isListLine(lines[i]) &&
                               !/^\s*```/.test(lines[i]) && !/^\s*#{1,6}\s/.test(lines[i]) && !/^\s*>/.test(lines[i]) &&
                               !/^\s*\$\$/.test(lines[i]) && !/^\s*\\\[/.test(lines[i]) && !/^\s*\$\s*$/.test(lines[i])) i++;
                        const para = lines.slice(start, i);
                        if (para.some(bookLineHasMath)) {
                            const url = await bookCaptureMarkdownFragment(para.join('\n'));
                            if (url) emit(url); else out.push(...para);
                        } else out.push(...para);
                    }
                    return { body: out.join('\n'), captures, hasDynamic: n > 0 };
                }

                async function generateBook() {
                    bookGenerating.value = true;
                    try {
                        // 1. Ask the server for the exact note set (+ content) the book will include.
                        const manifest = await fetch('/book/notes?scope=' + encodeURIComponent(bookScope.value))
                            .then(r => r.ok ? r.json() : []).catch(() => []);

                        // 2. Capture pass: render KaTeX/Mermaid in the browser (the single renderer) and
                        //    build per-note overlays. Mermaid uses a print-friendly theme during capture.
                        const overlays = {};
                        if (!window.htmlToImage) {
                            console.warn('Book: html-to-image not loaded — math/diagrams will fall back to raw source.');
                        }
                        if (window.htmlToImage && Array.isArray(manifest)) {
                            try { mermaid.initialize({ startOnLoad: false, theme: 'neutral', securityLevel: 'loose' }); } catch (e) {}
                            try {
                                let totalCaptures = 0;
                                for (const note of manifest) {
                                    try {
                                        const ov = await bookBuildOverlay(note.content);
                                        if (ov.hasDynamic) {
                                            overlays[note.path] = { body: ov.body, captures: ov.captures };
                                            totalCaptures += Object.keys(ov.captures).length;
                                        }
                                    } catch (e) { console.warn('Book: overlay failed for', note.path, e); }
                                }
                                console.info('Book: captured', totalCaptures, 'fragment image(s) across', Object.keys(overlays).length, 'note(s) of', manifest.length);
                            } finally {
                                try { mermaid.initialize({ startOnLoad: false, theme: 'dark' }); } catch (e) {}
                            }
                        }

                        // 3. Generate the PDF, handing over the captured fragments.
                        const res = await fetch('/book/generate', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({
                                title: bookTitle.value.trim() || 'KubiRAG Book',
                                eyebrow: bookEyebrow.value.trim(),
                                subtitle: bookSubtitle.value.trim(),
                                author: bookAuthor.value.trim(),
                                coverImage: bookCoverImage.value.trim(),
                                includeCover: bookIncludeCover.value,
                                includeToc: bookIncludeToc.value,
                                scope: bookScope.value,
                                outputDir: bookOutputDir.value.trim() || 'books',
                                overlays
                            })
                        });
                        if (!res.ok) {
                            const err = await res.json().catch(() => ({}));
                            throw new Error(err.error || `HTTP ${res.status}`);
                        }
                        const data = await res.json();
                        bookModalOpen.value = false;
                        window.open(data.url || `/book/${encodeURIComponent(data.filename)}`, '_blank');
                    } catch (e) {
                        alert('Book generation error: ' + e.message);
                    } finally {
                        bookGenerating.value = false;
                    }
                }

                // Browser-native book: opens the print page (paged.js) which renders KaTeX/Mermaid
                // natively and uses the browser's Save-as-PDF. Full fidelity, no server rasterizing.
                function printBook() {
                    const q = new URLSearchParams({
                        title: bookTitle.value.trim() || 'KubiRAG Book',
                        eyebrow: bookEyebrow.value.trim(),
                        subtitle: bookSubtitle.value.trim(),
                        author: bookAuthor.value.trim(),
                        coverImage: bookCoverImage.value.trim(),
                        scope: bookScope.value,
                        cover: bookIncludeCover.value ? '1' : '0',
                        toc: bookIncludeToc.value ? '1' : '0'
                    });
                    bookModalOpen.value = false;
                    window.open('/book/print?' + q.toString(), '_blank');
                }

                const { slideModalOpen, slideGenerating, slideOutputDir, openSlideModal, generateSlides } = useSlides();

                // --- Enrichment state ---
                const enrichmentRunning = ref(false);
                const enrichmentProgress = ref({ done: 0, total: 0, running: false, failed: 0, currentDocId: '', etaSeconds: -1 });
                const enrichScopeOpen = ref(false);
                const enrichScope = ref('all');

                // --- Evidence inbox state ---
                const evidencePanelOpen = ref(false);
                const evidenceLoading = ref(false);
                const evidenceDiscovering = ref(false);
                const okfBusy = ref(false);
                const evidenceItems = ref([]);
                const evidenceListTotal = ref(0);
                const evidenceFilter = ref('');
                const evidenceStatus = ref({ total: 0, pending: 0, accepted: 0, exported: 0, rejected: 0, path: '' });
                const evidenceMessage = ref('');
                const evidenceScope = ref('all'); // discovery scope: all | notes | documents | terms
                // One cross-panel display scope, like an Obsidian vault-level workspace state.
                // It is deliberately shared by Evidence and Concepts and survives modal closes.
                const knowledgeScopeMode = ref(localStorage.getItem('kubirag.knowledgeScope') || 'notebook');
                const activeNotebookScope = ref({ id: '', name: '' });
                async function loadKnowledgeScopeContext() {
                    try {
                        const res = await fetch('/governance', { cache: 'no-store' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        if (!data.id) throw new Error('The active notebook has no governed identity');
                        activeNotebookScope.value = { id: data.id, name: data.name || '' };
                        return true;
                    } catch (e) {
                        activeNotebookScope.value = { id: '', name: '' };
                        evidenceMessage.value = 'Notebook scope unavailable: ' + e.message;
                        console.warn('Notebook scope unavailable:', e.message);
                        return false;
                    }
                }
                function knowledgeScopeParams() {
                    const params = new URLSearchParams();
                    if (knowledgeScopeMode.value === 'notebook') {
                        if (!activeNotebookScope.value.id) {
                            throw new Error('Notebook scope unavailable: governed notebook identity was not loaded');
                        }
                        params.set('notebookId', activeNotebookScope.value.id);
                    }
                    return params;
                }
                async function changeKnowledgeScope() {
                    localStorage.setItem('kubirag.knowledgeScope', knowledgeScopeMode.value);
                    conceptDetail.value = null;
                    conceptIdentity.value = null;
                    if (evidencePanelOpen.value) await loadEvidence();
                    if (conceptsPanelOpen.value) await openConceptsPanel(conceptsView.value);
                }
                const conceptDetail = ref(null);
                const conceptDetailLoading = ref(false);
                const conceptIdentity = ref(null);
                const conceptPermalinkForm = ref({ slug: '', reason: '', preview: null });
                const conceptPermalinkBusy = ref(false);
                const termIdentities = ref([]);
                const termIdentity = ref(null);
                const termIdentityFilter = ref('');
                const termIdentitiesLoading = ref(false);
                const termPermalinkForm = ref({ slug: '', reason: '', preview: null });
                const termPermalinkBusy = ref(false);
                const filteredTermIdentities = computed(() => {
                    const q = termIdentityFilter.value.trim().toLowerCase();
                    return q ? termIdentities.value.filter(item =>
                        [item.label, item.uri, item.canonicalPermalink, ...(item.aliases || []), ...(item.permalinkAliases || [])]
                            .some(value => String(value || '').toLowerCase().includes(q))) : termIdentities.value;
                });
                let conceptDetailRequestId = 0;
                const conceptLinkForm = ref({ sourceId: '', relationType: 'mentions' });
                const conceptInsertForm = ref({ noteName: '' });
                const conceptAliasForm = ref({ alias: '' });
                const conceptRenameForm = ref({ newName: '' });
                const conceptRelationForm = ref({ predicateQuery: '', targetQuery: '', scope: 'notebook', predicate: null, target: null });
                const conceptPredicateOptions = ref([]);
                const conceptTargetOptions = ref([]);
                const conceptRelationBusy = ref(false);
                let conceptPredicateTimer = null;
                let conceptTargetTimer = null;
                const filteredEvidenceItems = computed(() => {
                    const q = evidenceFilter.value.trim().toLowerCase();
                    if (!q) return evidenceItems.value;
                    return evidenceItems.value.filter(item => {
                        const relation = item.relation
                            ? `${item.relation.subject || ''} ${item.relation.predicate || ''} ${item.relation.object || item.relation.object_ || ''}`
                            : '';
                        return [
                            item.concept,
                            item.signal,
                            item.sourceTitle,
                            item.sourcePath,
                            item.sourceId,
                            item.snippet,
                            relation
                        ].some(value => String(value || '').toLowerCase().includes(q));
                    });
                });

                // --- Concepts panel state (bilink [[ ]] view; reuses conceptDetail) ---
                const conceptsPanelOpen = ref(false);
                const conceptsLoading = ref(false);
                const conceptsList = ref([]);
                const conceptsFilter = ref('');
                const conceptsBilinkOnly = ref(false);
                const conceptTags = ref([]);
                const selectedConceptTag = ref('');
                const conceptRemoving = ref(false);
                const conceptRenaming = ref(false);
                const conceptsView = ref('list');
                const conceptMapDetail = ref('accepted');
                const conceptMapLoading = ref(false);
                const conceptMapError = ref('');
                const conceptMapData = ref(null);
                const conceptMapContainer = ref(null);
                const conceptMapSearch = ref('');
                const conceptMapFocusId = ref('');
                let conceptMapView = null;
                // Concepts present in the store, which is not the same as concepts received:
                // /concepts truncates to ?limit and reports the real size in X-Total-Count.
                const conceptsTotal = ref(0);
                const filteredConcepts = computed(() => {
                    let items = conceptsList.value;
                    if (conceptsBilinkOnly.value) items = items.filter(c => c.backlinkCount > 0);
                    const q = conceptsFilter.value.trim().toLowerCase();
                    if (q) items = items.filter(c => c.concept.toLowerCase().includes(q));
                    return items;
                });
                async function loadConceptTags() {
                    try {
                        const params = knowledgeScopeParams();
                        const res = await fetch('/concepts/tags?' + params.toString(), { cache: 'no-store' });
                        conceptTags.value = res.ok ? await res.json() : [];
                        if (selectedConceptTag.value && !conceptTags.value.some(t => t.name === selectedConceptTag.value)) {
                            selectedConceptTag.value = '';
                        }
                    } catch (e) {
                        conceptTags.value = [];
                    }
                }
                async function loadConceptList() {
                    const params = knowledgeScopeParams();
                    params.set('limit', '500');
                    if (selectedConceptTag.value) params.set('tag', selectedConceptTag.value);
                    const res = await fetch('/concepts?' + params.toString(), { cache: 'no-store' });
                    conceptsList.value = res.ok ? await res.json() : [];
                    conceptsTotal.value = Number(res.headers.get('X-Total-Count')) || conceptsList.value.length;
                }
                async function selectConceptTag(tag) {
                    selectedConceptTag.value = tag || '';
                    if (conceptMapDetail.value === 'affinity') {
                        conceptMapDetail.value = 'accepted';
                        conceptMapFocusId.value = '';
                    }
                    conceptsLoading.value = true;
                    conceptDetail.value = null;
                    conceptIdentity.value = null;
                    conceptPermalinkForm.value = { slug: '', reason: '', preview: null };
                    try {
                        await loadConceptList();
                    } catch (e) {
                        conceptsList.value = [];
                        conceptsTotal.value = 0;
                        evidenceMessage.value = 'Concepts error: ' + e.message;
                    } finally {
                        conceptsLoading.value = false;
                    }
                    if (conceptsView.value === 'map') await loadConceptMap();
                }
                async function openConceptsPanel(view = 'list') {
                    // A bare Vue @click handler receives the DOM event as its first argument.
                    // Only the explicit map value may override the default list view.
                    view = ['map', 'terms'].includes(view) ? view : 'list';
                    conceptsPanelOpen.value = true;
                    conceptsView.value = view;
                    conceptDetailRequestId += 1;
                    conceptDetail.value = null;
                    conceptDetailLoading.value = false;
                    conceptsLoading.value = true;
                    try {
                        await loadKnowledgeScopeContext();
                        if (view === 'terms') await loadTermIdentities();
                        else {
                            await loadConceptTags();
                            await loadConceptList();
                        }
                    } catch (e) {
                        conceptsList.value = [];
                        conceptsTotal.value = 0;
                        evidenceMessage.value = 'Concepts error: ' + e.message;
                    } finally {
                        conceptsLoading.value = false;
                    }
                    if (view === 'map') await loadConceptMap();
                }

                async function selectConceptsView(view) {
                    conceptsView.value = view;
                    conceptMapError.value = '';
                    if (view === 'map') {
                        await loadConceptMap();
                    } else if (view === 'terms') {
                        conceptMapView?.destroy();
                        conceptMapView = null;
                        await loadTermIdentities();
                    } else {
                        conceptMapView?.destroy();
                        conceptMapView = null;
                    }
                }

                async function loadTermIdentities() {
                    termIdentitiesLoading.value = true;
                    termIdentity.value = null;
                    termPermalinkForm.value = { slug: '', reason: '', preview: null };
                    try {
                        const res = await fetch('/resources/search?kind=term&limit=500', { cache: 'no-store' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        termIdentities.value = data;
                    } catch (e) {
                        termIdentities.value = [];
                        evidenceMessage.value = 'Term identity error: ' + e.message;
                    } finally {
                        termIdentitiesLoading.value = false;
                    }
                }

                function selectTermIdentity(item) {
                    termIdentity.value = item;
                    termPermalinkForm.value = { slug: '', reason: '', preview: null };
                }

                async function previewTermPermalink() {
                    const slug = termPermalinkForm.value.slug.trim();
                    termPermalinkForm.value.preview = null;
                    if (!slug) return;
                    try {
                        const res = await fetch('/resources/permalinks/preview?' + new URLSearchParams({ kind: 'term', slug }), { cache: 'no-store' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        termPermalinkForm.value.preview = data;
                    } catch (e) {
                        evidenceMessage.value = 'Permalink preview error: ' + e.message;
                    }
                }

                async function promoteTermPermalink() {
                    const preview = termPermalinkForm.value.preview;
                    const reason = termPermalinkForm.value.reason.trim();
                    if (!termIdentity.value || !preview?.available || !reason || termPermalinkBusy.value) return;
                    if (!confirm(`Publish ${preview.permalink}? Published permalinks can never be reassigned.`)) return;
                    termPermalinkBusy.value = true;
                    try {
                        const statusRes = await fetch('/resources/identity/status', { cache: 'no-store' });
                        const status = await statusRes.json();
                        if (!statusRes.ok) throw new Error(status.error || `HTTP ${statusRes.status}`);
                        const res = await fetch('/resources/permalinks', {
                            method: 'POST', headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({
                                resourceUri: termIdentity.value.uri,
                                slug: termPermalinkForm.value.slug.trim(),
                                expectedRevision: status.registryRevision,
                                actor: 'ragboard-user', reason,
                                operationId: crypto.randomUUID ? crypto.randomUUID() : `ragboard-${Date.now()}`
                            })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        const selectedUri = termIdentity.value.uri;
                        await loadTermIdentities();
                        termIdentity.value = termIdentities.value.find(item => item.uri === selectedUri) || null;
                        evidenceMessage.value = 'Term permalink published. Previous permalinks remain active aliases.';
                    } catch (e) {
                        evidenceMessage.value = 'Permalink promotion error: ' + e.message;
                    } finally {
                        termPermalinkBusy.value = false;
                    }
                }

                async function loadConceptMap() {
                    if (conceptMapDetail.value === 'affinity' && !conceptMapFocusId.value) {
                        conceptMapError.value = 'Select a concept before enabling affinity.';
                        return;
                    }
                    conceptMapLoading.value = true;
                    conceptMapError.value = '';
                    try {
                        const params = new URLSearchParams({ detail: conceptMapDetail.value });
                        knowledgeScopeParams().forEach((value, key) => params.set(key, value));
                        if (selectedConceptTag.value) params.set('tag', selectedConceptTag.value);
                        if (conceptMapDetail.value === 'affinity') params.set('focus', conceptMapFocusId.value);
                        const res = await fetch('/concepts/graph?' + params.toString(), { cache: 'no-store' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        conceptMapData.value = data;
                        if (data.focus) conceptMapFocusId.value = data.focus;
                        await nextTick();
                        conceptMapView?.destroy();
                        conceptMapView = conceptMapContainer.value
                            ? renderConceptMap(
                                conceptMapContainer.value,
                                { ...data, focus: data.focus || conceptMapFocusId.value },
                                { onSelect: selectConceptMapNode }
                            )
                            : null;
                    } catch (e) {
                        conceptMapData.value = null;
                        conceptMapView?.destroy();
                        conceptMapView = null;
                        conceptMapError.value = 'Concept map error: ' + e.message;
                    } finally {
                        conceptMapLoading.value = false;
                    }
                }

                async function selectConceptMapDetail() {
                    await loadConceptMap();
                }

                async function selectConceptMapNode(node) {
                    conceptMapFocusId.value = node.id;
                    await openConceptDetail(node.name);
                    if (conceptMapDetail.value === 'affinity') {
                        await loadConceptMap();
                    } else {
                        conceptMapView?.focusNode(node.id);
                    }
                }

                async function focusConceptMap() {
                    const query = conceptMapSearch.value.trim().toLowerCase();
                    if (!query) return;
                    const mapNode = conceptMapData.value?.nodes?.find(node =>
                        node.name.toLowerCase() === query ||
                        (node.aliases || []).some(alias => alias.toLowerCase() === query)
                    ) || conceptMapData.value?.nodes?.find(node =>
                        node.name.toLowerCase().includes(query) ||
                        (node.aliases || []).some(alias => alias.toLowerCase().includes(query))
                    );
                    const conceptName = mapNode?.name || conceptsList.value.find(item =>
                        item.concept.toLowerCase() === query
                    )?.concept || conceptsList.value.find(item => item.concept.toLowerCase().includes(query))?.concept;
                    if (!conceptName) {
                        conceptMapError.value = `No concept matches "${conceptMapSearch.value.trim()}".`;
                        return;
                    }
                    conceptMapError.value = '';
                    conceptMapFocusId.value = mapNode?.id || conceptName;
                    await openConceptDetail(conceptName);
                    if (conceptMapDetail.value === 'affinity') {
                        await loadConceptMap();
                    } else if (mapNode) {
                        conceptMapView?.focusNode(mapNode.id);
                    }
                }

                function fitConceptMap() {
                    conceptMapView?.fit();
                }

                async function refreshConceptMapIfVisible() {
                    if (conceptsPanelOpen.value && conceptsView.value === 'map') await loadConceptMap();
                }

                watch(conceptsPanelOpen, open => {
                    if (!open) {
                        conceptMapView?.destroy();
                        conceptMapView = null;
                    }
                });

                async function loadEvidenceStatus() {
                    try {
                        const params = knowledgeScopeParams();
                        const res = await fetch('/evidence/status' + (params.size ? '?' + params.toString() : ''));
                        if (res.ok) evidenceStatus.value = await res.json();
                    } catch (e) {
                        console.warn('Evidence status unavailable:', e.message);
                    }
                }

                async function loadEvidence() {
                    evidenceLoading.value = true;
                    try {
                        await loadKnowledgeScopeContext();
                        const scopeParams = knowledgeScopeParams();
                        const pendingParams = new URLSearchParams(scopeParams);
                        const acceptedParams = new URLSearchParams(scopeParams);
                        pendingParams.set('status', 'pending');
                        acceptedParams.set('status', 'accepted');
                        // Load pending (to review) AND accepted evidence.
                        const [pend, acc] = await Promise.all([
                            fetch('/evidence?' + pendingParams.toString()),
                            fetch('/evidence?' + acceptedParams.toString())
                        ]);
                        const pendItems = pend.ok ? await pend.json() : [];
                        const accItems = acc.ok ? await acc.json() : [];
                        evidenceItems.value = [...pendItems, ...accItems];
                        const pendingTotal = Number(pend.headers.get('X-Total-Count')) || pendItems.length;
                        const acceptedTotal = Number(acc.headers.get('X-Total-Count')) || accItems.length;
                        evidenceListTotal.value = pendingTotal + acceptedTotal;
                        await loadEvidenceStatus();
                    } catch (e) {
                        evidenceItems.value = [];
                        evidenceListTotal.value = 0;
                    } finally {
                        evidenceLoading.value = false;
                    }
                }

                async function openEvidencePanel() {
                    evidencePanelOpen.value = true;
                    await loadEvidence();
                }

                async function discoverEvidence() {
                    evidenceDiscovering.value = true;
                    evidenceMessage.value = '';
                    try {
                        const res = await fetch('/evidence/discover?scope=' + encodeURIComponent(evidenceScope.value), { method: 'POST' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        evidenceMessage.value = `Added ${data.added}, total ${data.total}`;
                        await loadEvidence();
                    } catch (e) {
                        evidenceMessage.value = 'Discovery error: ' + e.message;
                    } finally {
                        evidenceDiscovering.value = false;
                    }
                }

                async function addWebEvidence() {
                    const concept = window.prompt('Concept name:', conceptDetail.value?.summary?.concept || 'Knowledge Chunk');
                    if (!concept) return;
                    const url = window.prompt('Evidence URL:', 'https://en.wikipedia.org/wiki/Chunking_(psychology)');
                    if (!url) return;
                    const title = window.prompt('Evidence title:', 'Chunking (psychology)') || url;
                    try {
                        const res = await fetch('/evidence/web', {
                            method: 'POST', headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ concept, url, title, relation: 'related' })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        evidenceMessage.value = `Web evidence ${data.id} added as pending.`;
                        await loadEvidence();
                    } catch (e) { evidenceMessage.value = 'Web evidence error: ' + e.message; }
                }

                // Export the concepts as an OKF bundle (concepts + source stubs + index.md + log.md).
                async function okfExport() {
                    okfBusy.value = true;
                    evidenceMessage.value = '';
                    try {
                        const res = await fetch('/concepts/okf/export', { method: 'POST' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        evidenceMessage.value = `OKF export: ${data.conceptFiles} concepts + ${data.sourceFiles} sources → ${data.bundleDir}`;
                    } catch (e) {
                        evidenceMessage.value = 'OKF export error: ' + e.message;
                    } finally {
                        okfBusy.value = false;
                    }
                }

                // Import an OKF bundle directory (server-side path) into the concept stores.
                async function okfImport() {
                    const dir = window.prompt('OKF bundle directory to import (path on the server):');
                    if (!dir) return;
                    okfBusy.value = true;
                    evidenceMessage.value = '';
                    try {
                        const res = await fetch('/concepts/okf/import?dir=' + encodeURIComponent(dir), { method: 'POST' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        evidenceMessage.value = `OKF import: ${data.imported} concepts, ${data.sourcesResolved} sources, ${data.skipped} skipped`;
                        await openConceptsPanel();
                    } catch (e) {
                        evidenceMessage.value = 'OKF import error: ' + e.message;
                    } finally {
                        okfBusy.value = false;
                    }
                }

                async function rejectEvidence(item) {
                    try {
                        const res = await fetch(`/evidence/${encodeURIComponent(item.id)}/reject`, { method: 'POST' });
                        if (!res.ok) throw new Error(`HTTP ${res.status}`);
                        evidenceItems.value = evidenceItems.value.filter(e => e.id !== item.id);
                        await loadEvidenceStatus();
                    } catch (e) {
                        evidenceMessage.value = 'Reject error: ' + e.message;
                    }
                }

                async function acceptEvidence(item) {
                    try {
                        const res = await fetch(`/evidence/${encodeURIComponent(item.id)}/accept`, { method: 'POST' });
                        if (!res.ok) throw new Error(`HTTP ${res.status}`);
                        // Accepted evidence stays in the panel with a status badge.
                        item.status = 'accepted';
                        evidenceMessage.value = `Accepted evidence for "${item.concept}".`;
                        await loadEvidenceStatus();
                    } catch (e) {
                        evidenceMessage.value = 'Accept evidence error: ' + e.message;
                    }
                }

                async function writeConceptNote(item) {
                    evidenceMessage.value = '';
                    try {
                        const res = await fetch('/concepts/note', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ concept: item.concept })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        evidenceMessage.value = `${data.created ? 'Created' : 'Updated'} concept note: ${data.path}`;
                        if (conceptDetail.value && conceptDetail.value.summary.concept === item.concept) {
                            conceptDetail.value.noteExists = true;
                            conceptDetail.value.notePath = data.path;
                        }
                        // Creating a concept note must not silently change the globally browsed
                        // notes folder: doing so replaces the current sidebar list and can make it
                        // appear empty. Refresh the current view and the recursive note index; if
                        // the user is already browsing Concepts, the new file appears there.
                        if (data.created) leftTab.value = 'notes';
                        await loadFolders();
                        await loadNotes();
                    } catch (e) {
                        evidenceMessage.value = 'Concept note error: ' + e.message;
                    }
                }

                function editConceptNote(item) {
                    const concept = typeof item === 'string' ? item : item.concept;
                    conceptsPanelOpen.value = false;
                    evidencePanelOpen.value = false;
                    window.location.href = `/markboard?note=${encodeURIComponent(concept)}`;
                }

                async function openConceptDetail(item) {
                    const concept = typeof item === 'string' ? item : item.concept;
                    const requestId = ++conceptDetailRequestId;
                    conceptDetailLoading.value = true;
                    // Do not leave sources/manual links from the previous concept visible while
                    // the new detail is loading. The request id also prevents an older, slower
                    // response from replacing the most recently selected concept.
                    conceptDetail.value = null;
                    conceptIdentity.value = null;
                    conceptPermalinkForm.value = { slug: '', reason: '', preview: null };
                    conceptLinkForm.value = { sourceId: '', relationType: 'mentions' };
                    conceptInsertForm.value = { noteName: '' };
                    conceptAliasForm.value = { alias: '' };
                    conceptRenameForm.value = { newName: '' };
                    conceptRelationForm.value = { predicateQuery: '', targetQuery: '', scope: 'notebook', predicate: null, target: null };
                    conceptPredicateOptions.value = [];
                    conceptTargetOptions.value = [];
                    evidenceMessage.value = '';
                    try {
                        const params = knowledgeScopeParams();
                        const suffix = params.size ? '?' + params.toString() : '';
                        const res = await fetch(`/concepts/${encodeURIComponent(concept)}${suffix}`, { cache: 'no-store' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        if (requestId !== conceptDetailRequestId) return;
                        conceptDetail.value = data;
                        await Promise.all([searchConceptPredicates(true), searchConceptTargets(true), loadConceptIdentity()]);
                    } catch (e) {
                        if (requestId !== conceptDetailRequestId) return;
                        evidenceMessage.value = 'Concept detail error: ' + e.message;
                    } finally {
                        if (requestId === conceptDetailRequestId) conceptDetailLoading.value = false;
                    }
                }

                async function loadConceptIdentity() {
                    conceptIdentity.value = null;
                    const uri = conceptDetail.value?.conceptUri;
                    if (!uri) return;
                    const res = await fetch('/resources/resolve?uri=' + encodeURIComponent(uri), { cache: 'no-store' });
                    if (res.status === 404) return;
                    const data = await res.json();
                    if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                    conceptIdentity.value = data;
                }

                async function previewConceptPermalink() {
                    const slug = conceptPermalinkForm.value.slug.trim();
                    conceptPermalinkForm.value.preview = null;
                    if (!slug) return;
                    try {
                        const params = new URLSearchParams({ kind: 'concept', slug });
                        const res = await fetch('/resources/permalinks/preview?' + params.toString(), { cache: 'no-store' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        conceptPermalinkForm.value.preview = data;
                    } catch (e) {
                        evidenceMessage.value = 'Permalink preview error: ' + e.message;
                    }
                }

                async function promoteConceptPermalink() {
                    const preview = conceptPermalinkForm.value.preview;
                    const reason = conceptPermalinkForm.value.reason.trim();
                    if (!conceptIdentity.value || !preview || !preview.available || !reason || conceptPermalinkBusy.value) return;
                    if (!confirm(`Publish ${preview.permalink}? Published permalinks can never be reassigned.`)) return;
                    conceptPermalinkBusy.value = true;
                    try {
                        const statusRes = await fetch('/resources/identity/status', { cache: 'no-store' });
                        const status = await statusRes.json();
                        if (!statusRes.ok) throw new Error(status.error || `HTTP ${statusRes.status}`);
                        const res = await fetch('/resources/permalinks', {
                            method: 'POST', headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({
                                resourceUri: conceptIdentity.value.uri,
                                slug: conceptPermalinkForm.value.slug.trim(),
                                expectedRevision: status.registryRevision,
                                actor: 'ragboard-user', reason,
                                operationId: crypto.randomUUID ? crypto.randomUUID() : `ragboard-${Date.now()}`
                            })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        conceptPermalinkForm.value = { slug: '', reason: '', preview: null };
                        await loadConceptIdentity();
                        evidenceMessage.value = 'Permalink published. Previous permalinks remain active aliases.';
                    } catch (e) {
                        evidenceMessage.value = 'Permalink promotion error: ' + e.message;
                    } finally {
                        conceptPermalinkBusy.value = false;
                    }
                }

                async function copyIdentity(value) {
                    if (!value) return;
                    await navigator.clipboard.writeText(value);
                    evidenceMessage.value = 'Identity copied to clipboard.';
                }

                async function searchConceptPredicates(immediate = false) {
                    clearTimeout(conceptPredicateTimer);
                    const run = async () => {
                        try {
                            const params = new URLSearchParams({ q: conceptRelationForm.value.predicateQuery, limit: '40' });
                            const res = await fetch('/concepts/predicates?' + params.toString(), { cache: 'no-store' });
                            const data = await res.json();
                            if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                            conceptPredicateOptions.value = data;
                        } catch (e) {
                            conceptPredicateOptions.value = [];
                            evidenceMessage.value = 'Predicate catalog error: ' + e.message;
                        }
                    };
                    if (immediate) await run(); else conceptPredicateTimer = setTimeout(run, 220);
                }

                async function searchConceptTargets(immediate = false) {
                    clearTimeout(conceptTargetTimer);
                    if (!conceptDetail.value) return;
                    const run = async () => {
                        try {
                            const form = conceptRelationForm.value;
                            const params = new URLSearchParams({
                                q: form.targetQuery,
                                scope: form.scope,
                                excludeId: conceptDetail.value.conceptId,
                                limit: '40'
                            });
                            if (form.scope === 'notebook') {
                                if (!activeNotebookScope.value.id) {
                                    throw new Error('governed notebook identity was not loaded');
                                }
                                params.set('notebookId', activeNotebookScope.value.id);
                            }
                            const res = await fetch('/concepts/targets?' + params.toString(), { cache: 'no-store' });
                            const data = await res.json();
                            if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                            conceptTargetOptions.value = data;
                        } catch (e) {
                            conceptTargetOptions.value = [];
                            evidenceMessage.value = 'Concept target error: ' + e.message;
                        }
                    };
                    if (immediate) await run(); else conceptTargetTimer = setTimeout(run, 220);
                }

                function selectConceptPredicate(predicate) {
                    conceptRelationForm.value.predicate = predicate;
                    conceptRelationForm.value.predicateQuery = predicate.qname;
                }

                function selectConceptTarget(target) {
                    conceptRelationForm.value.target = target;
                    conceptRelationForm.value.targetQuery = target.name;
                }

                function conceptPredicateLabel(rel) {
                    const iri = rel.predicateUri || rel.predicate;
                    return conceptPredicateOptions.value.find(item => item.iri === iri)?.qname || rel.predicate;
                }

                async function addConceptRelation() {
                    const detail = conceptDetail.value;
                    const form = conceptRelationForm.value;
                    if (!detail || !form.predicate || !form.target || conceptRelationBusy.value) return;
                    conceptRelationBusy.value = true;
                    try {
                        const res = await fetch(`/concepts/${encodeURIComponent(detail.conceptId)}/relations`, {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({
                                predicateUri: form.predicate.iri,
                                objectUri: form.target.uri,
                                expectedRevision: detail.revision
                            })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        const concept = detail.summary.concept;
                        evidenceMessage.value = data.changed ? 'Concept relation added.' : 'Concept relation already exists.';
                        await openConceptDetail(concept);
                        await loadConceptList();
                        await refreshConceptMapIfVisible();
                    } catch (e) {
                        evidenceMessage.value = 'Concept relation error: ' + e.message;
                    } finally {
                        conceptRelationBusy.value = false;
                    }
                }

                async function removeConceptRelation(rel) {
                    const detail = conceptDetail.value;
                    if (!detail || conceptRelationBusy.value) return;
                    conceptRelationBusy.value = true;
                    try {
                        const subjectId = rel.subjectId || detail.conceptId;
                        const res = await fetch(`/concepts/${encodeURIComponent(subjectId)}/relations`, {
                            method: 'DELETE',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({
                                predicateUri: rel.predicateUri || rel.predicate,
                                objectUri: rel.objectUri,
                                expectedRevision: detail.revision
                            })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        const concept = detail.summary.concept;
                        evidenceMessage.value = data.changed ? 'Concept relation removed.' : 'Concept relation was already absent.';
                        await openConceptDetail(concept);
                        await loadConceptList();
                        await refreshConceptMapIfVisible();
                    } catch (e) {
                        evidenceMessage.value = 'Concept relation removal error: ' + e.message;
                    } finally {
                        conceptRelationBusy.value = false;
                    }
                }

                async function linkConceptSource() {
                    if (!conceptDetail.value || !conceptLinkForm.value.sourceId.trim()) return;
                    try {
                        const concept = conceptDetail.value.summary.concept;
                        const sourceId = conceptLinkForm.value.sourceId.trim();
                        const res = await fetch('/concepts/link', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({
                                concept,
                                sourceId,
                                sourcePath: sourceId,
                                relationType: conceptLinkForm.value.relationType,
                                sourceType: 'document'
                            })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        evidenceMessage.value = `Linked ${concept} -> ${sourceId}`;
                        await openConceptDetail(concept);
                        await refreshConceptMapIfVisible();
                    } catch (e) {
                        evidenceMessage.value = 'Concept link error: ' + e.message;
                    }
                }

                async function unlinkConceptSource(link) {
                    if (!conceptDetail.value) return;
                    try {
                        const concept = conceptDetail.value.summary.concept;
                        const res = await fetch('/concepts/unlink', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ concept, sourceId: link.sourceId })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        evidenceMessage.value = `Unlinked ${concept} -> ${link.sourceId}`;
                        await openConceptDetail(concept);
                        await refreshConceptMapIfVisible();
                    } catch (e) {
                        evidenceMessage.value = 'Concept unlink error: ' + e.message;
                    }
                }

                async function insertConceptWikiLink() {
                    if (!conceptDetail.value || !conceptInsertForm.value.noteName.trim()) return;
                    try {
                        const concept = conceptDetail.value.summary.concept;
                        const noteName = conceptInsertForm.value.noteName.trim();
                        const res = await fetch('/concepts/insert', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ concept, noteName })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        evidenceMessage.value = data.alreadyPresent
                            ? `Wiki link already present in ${data.noteName}`
                            : `Inserted wiki link in ${data.noteName}`;
                    } catch (e) {
                        evidenceMessage.value = 'Concept insert error: ' + e.message;
                    }
                }

                async function addConceptAlias() {
                    if (!conceptDetail.value || !conceptAliasForm.value.alias.trim()) return;
                    try {
                        const concept = conceptDetail.value.summary.concept;
                        const alias = conceptAliasForm.value.alias.trim();
                        const res = await fetch('/concepts/alias', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ concept, alias })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        evidenceMessage.value = data.added
                            ? `Alias added: ${data.alias}`
                            : `Alias already present: ${data.alias}`;
                        await openConceptDetail(concept);
                        await refreshConceptMapIfVisible();
                    } catch (e) {
                        evidenceMessage.value = 'Concept alias error: ' + e.message;
                    }
                }

                async function removeConceptAlias(alias) {
                    if (!conceptDetail.value) return;
                    try {
                        const concept = conceptDetail.value.summary.concept;
                        const res = await fetch('/concepts/unalias', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ concept, alias })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        evidenceMessage.value = `Alias removed: ${alias}`;
                        await openConceptDetail(concept);
                        await refreshConceptMapIfVisible();
                    } catch (e) {
                        evidenceMessage.value = 'Concept unalias error: ' + e.message;
                    }
                }

                async function renameSelectedConcept() {
                    if (!conceptDetail.value || conceptRenaming.value) return;
                    const concept = conceptDetail.value.summary.concept;
                    const newName = conceptRenameForm.value.newName.trim();
                    if (!newName || newName === concept) return;
                    conceptRenaming.value = true;
                    try {
                        const res = await fetch('/concepts/rename', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ concept, newName })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        conceptRenameForm.value = { newName: '' };
                        await openConceptsPanel(conceptsView.value);
                        await openConceptDetail(data.newName);
                        await refreshConceptMapIfVisible();
                        conceptPermalinkForm.value.slug = data.newName;
                        await previewConceptPermalink();
                        evidenceMessage.value = `Concept renamed: ${data.oldName} → ${data.newName}. Existing permalinks remain active; promote the new name explicitly if desired.`;
                    } catch (e) {
                        evidenceMessage.value = 'Concept rename error: ' + e.message;
                    } finally {
                        conceptRenaming.value = false;
                    }
                }

                async function removeSelectedConcept() {
                    if (!conceptDetail.value || conceptRemoving.value) return;
                    const concept = conceptDetail.value.summary.concept;
                    const confirmed = confirm(
                        `Remove concept "${concept}"?\n\n` +
                        'Its evidence will be rejected and its governed relations and aliases removed. ' +
                        'The concept note and wiki-links in source notes will be left untouched.'
                    );
                    if (!confirmed) return;
                    conceptRemoving.value = true;
                    try {
                        const res = await fetch(`/concepts/${encodeURIComponent(concept)}`, { method: 'DELETE' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        conceptDetailRequestId += 1;
                        conceptDetail.value = null;
                        conceptMapFocusId.value = '';
                        if (conceptMapDetail.value === 'affinity') conceptMapDetail.value = 'accepted';
                        await openConceptsPanel(conceptsView.value);
                        await loadEvidenceStatus();
                        evidenceMessage.value = `Concept removed: ${data.concept} · ` +
                            `${data.rejectedEvidence} evidence rejected · ` +
                            `${data.removedManualLinks} links removed · ${data.removedAliases} aliases removed`;
                    } catch (e) {
                        evidenceMessage.value = 'Concept removal error: ' + e.message;
                    } finally {
                        conceptRemoving.value = false;
                    }
                }

                async function acceptConceptSuggestion(rel) {
                    if (!conceptDetail.value || !rel.evidenceId) return;
                    try {
                        const concept = conceptDetail.value.summary.concept;
                        const res = await fetch(`/concepts/suggestions/${encodeURIComponent(rel.evidenceId)}/accept`, { method: 'POST' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        evidenceMessage.value = `Accepted suggestion: ${data.concept}`;
                        await openConceptDetail(concept);
                        await loadEvidenceStatus();
                        await refreshConceptMapIfVisible();
                    } catch (e) {
                        evidenceMessage.value = 'Concept suggestion accept error: ' + e.message;
                    }
                }

                async function rejectConceptSuggestion(rel) {
                    if (!conceptDetail.value || !rel.evidenceId) return;
                    try {
                        const concept = conceptDetail.value.summary.concept;
                        const res = await fetch(`/concepts/suggestions/${encodeURIComponent(rel.evidenceId)}/reject`, { method: 'POST' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        evidenceMessage.value = `Rejected suggestion: ${rel.evidenceId}`;
                        await openConceptDetail(concept);
                        await loadEvidenceStatus();
                        await refreshConceptMapIfVisible();
                    } catch (e) {
                        evidenceMessage.value = 'Concept suggestion reject error: ' + e.message;
                    }
                }

                function openEnrichModal() {
                    enrichScopeOpen.value = true;
                }

                async function confirmStartEnrichment() {
                    enrichScopeOpen.value = false;
                    await startEnrichment(enrichScope.value);
                }

                async function startEnrichment(documentScope = 'all') {
                    const params = new URLSearchParams({ scope: documentScope });
                    await fetch('/enrich/start?' + params.toString(), { method: 'POST' });
                    enrichmentRunning.value = true;
                }
                async function stopEnrichment() {
                    await fetch('/enrich/stop', { method: 'POST' });
                    enrichmentRunning.value = false;
                }

                let enrichPollTimer = null;
                watch(enrichmentRunning, (running) => {
                    clearInterval(enrichPollTimer);
                    if (!running) return;
                    enrichPollTimer = setInterval(async () => {
                        try {
                            const res = await fetch('/enrich/status');
                            const data = await res.json();
                            enrichmentProgress.value = data;
                            if (!data.running) {
                                enrichmentRunning.value = false;
                                clearInterval(enrichPollTimer);
                                await refreshSources();  // final refresh to update all ● markers
                                await loadNotes();       // show new enrichment report in notes panel
                                await loadEvidenceStatus();
                            } else {
                                await refreshSources();  // live marker updates
                            }
                        } catch (e) { console.warn('Enrichment poll failed:', e.message); }
                    }, 2000);
                });

                // --- Sunburst state ---
                const sunburstOpen = ref(false);
                const sunburstLoading = ref(false);
                const sunburstData = ref(null);
                const sunburstError = ref('');
                const sunburstContainer = ref(null);
                const sunburstBreadcrumb = ref([]);  // [{label, term}] navigation stack
                const sunburstScope = ref('auto');
                const sunburstMode = ref('lsa');
                const sunburstTopics = ref(null);
                const sunburstDepth = ref(3);
                const sunburstGranularity = ref('auto');
                const sunburstSelected = ref(null);

                async function fetchAndRenderSunburst(term) {
                    sunburstLoading.value = true;
                    sunburstError.value = '';
                    sunburstSelected.value = null;
                    try {
                        const params = new URLSearchParams();
                        if (term) params.set('term', term);
                        params.set('scope', sunburstScope.value);
                        params.set('mode', sunburstMode.value);
                        if ((sunburstMode.value === 'lsa' || sunburstMode.value === 'kmeans') && sunburstTopics.value) {
                            params.set('topics', String(sunburstTopics.value));
                        }
                        if (sunburstMode.value === 'lsa') {
                            params.set('depth', String(sunburstDepth.value || 3));
                            if (sunburstGranularity.value && sunburstGranularity.value !== 'auto') {
                                params.set('granularity', sunburstGranularity.value);
                            }
                        }
                        const res = await fetch(`/mindmap?${params.toString()}`);
                        if (!res.ok) throw new Error(`HTTP ${res.status}`);
                        const data = await res.json();
                        sunburstData.value = data;
                        sunburstLoading.value = false;
                        await nextTick();
                        if (sunburstContainer.value && data.hierarchy) {
                            renderSunburst(sunburstContainer.value, data.hierarchy, {
                                canBack: sunburstBreadcrumb.value.length > 1,
                                onBack: sunburstBack,
                                onSelect: node => { sunburstSelected.value = node; },
                                onFocus: node => focusSunburstNode(node)
                            });
                        }
                    } catch (e) {
                        sunburstError.value = 'Error: ' + e.message;
                        sunburstLoading.value = false;
                    }
                }

                async function openSunburst() {
                    sunburstOpen.value = true;
                    sunburstBreadcrumb.value = [{ label: 'Corpus', term: null }];
                    await fetchAndRenderSunburst(null);
                }

                async function focusSunburstNode(node) {
                    const term = node && (node.term || node.name);
                    if (!term) return;
                    sunburstBreadcrumb.value.push({ label: node.name || term, term });
                    await fetchAndRenderSunburst(term);
                }

                async function sunburstBack() {
                    if (sunburstBreadcrumb.value.length <= 1) return;
                    await sunburstNavigateTo(sunburstBreadcrumb.value.length - 2);
                }

                async function sunburstNavigateTo(idx) {
                    const crumb = sunburstBreadcrumb.value[idx];
                    sunburstBreadcrumb.value = sunburstBreadcrumb.value.slice(0, idx + 1);
                    await fetchAndRenderSunburst(crumb.term);
                }

                function refreshSunburst() {
                    const active = sunburstBreadcrumb.value[sunburstBreadcrumb.value.length - 1] || { term: null };
                    return fetchAndRenderSunburst(active.term);
                }

                function openSunburstDocument(doc) {
                    sunburstOpen.value = false;
                    viewDoc(doc);
                }

                async function openEvidenceSource(source) {
                    if (!source) return;
                    const sourceId = source.sourceId || source.id || '';
                    const sourcePath = source.sourcePath || source.path || '';
                    const sourceTitle = source.sourceTitle || source.filename || '';
                    // Note-sourced evidence ([[ ]] found in a note) lives in the Notes panel, not in documents.
                    if (source.sourceType === 'note' || sourceId.startsWith('note:')) {
                        const rel = sourceId.startsWith('note:') ? sourceId.slice('note:'.length) : (sourcePath || sourceTitle);
                        const noteName = String(rel).split(/[\\/]/).pop().replace(/\.(md|html)$/i, '');
                        if (noteName) {
                            evidencePanelOpen.value = false;
                            await viewNote({ name: noteName, kind: 'note' });
                            return;
                        }
                    }
                    let doc = sources.value.find(d =>
                        d.id === sourceId ||
                        d.path === sourcePath ||
                        d.filename === sourceTitle ||
                        d.filename === sourceId ||
                        d.path === sourceId
                    );
                    if (!doc) {
                        await refreshSources();
                        doc = sources.value.find(d =>
                            d.id === sourceId ||
                            d.path === sourcePath ||
                            d.filename === sourceTitle ||
                            d.filename === sourceId ||
                            d.path === sourceId
                        );
                    }
                    if (!doc) {
                        evidenceMessage.value = `Source not found: ${sourceTitle || sourcePath || sourceId}`;
                        return;
                    }
                    evidencePanelOpen.value = false;
                    viewDoc(doc);
                }

                function renderSunburst(container, hierarchyData, handlers) {
                    container.innerHTML = '';
                    // Fixed logical coordinate space — SVG stretches to fill container via CSS
                    const vbSize = 600;
                    const radius = vbSize / 2;

                    const palette = ['#5b8def', '#49a078', '#f2c14e', '#ef476f', '#9b7ede',
                        '#4cc9f0', '#f77f00', '#80b918', '#ffafcc', '#b8bedd'];
                    const color = d3.scaleOrdinal(palette);

                    const layoutData = normalizeSunburstLayout(hierarchyData);
                    const hier = d3.hierarchy(layoutData)
                        .sum(d => Math.max(d.layoutValue || 1, 1))
                        .sort((a, b) => b.value - a.value);

                    const root = d3.partition().size([2 * Math.PI, hier.height + 1])(hier);
                    root.each(d => d.current = d);

                    const ringWidth = radius / (hier.height + 1);
                    const arc = d3.arc()
                        .startAngle(d => d.x0)
                        .endAngle(d => d.x1)
                        .padAngle(d => Math.min((d.x1 - d.x0) / 2, 0.005))
                        .padRadius(radius * 1.5)
                        .innerRadius(d => Math.max(0, d.y0) * ringWidth)
                        .outerRadius(d => Math.max(0, d.y1 - 0.5) * ringWidth);

                    const svg = d3.select(container).append('svg')
                        .attr('viewBox', `${-radius} ${-radius} ${vbSize} ${vbSize}`)
                        .attr('preserveAspectRatio', 'xMidYMid meet')
                        .style('width', '100%').style('height', '100%').style('display', 'block')
                        .style('font', '11px sans-serif');

                    function arcVisible(d) { return d.y1 <= hier.height + 1 && d.y0 >= 1 && d.x1 > d.x0; }
                    function labelVisible(d) { return arcVisible(d) && (d.y1 - d.y0) * (d.x1 - d.x0) > 0.03; }
                    function labelTransform(d) {
                        const x = (d.x0 + d.x1) / 2 * 180 / Math.PI;
                        const y = (d.y0 + d.y1) / 2 * ringWidth;
                        return `rotate(${x - 90}) translate(${y},0) rotate(${x < 180 ? 0 : 180})`;
                    }
                    function nodeValueText(d) {
                        const label = d.data.valueLabel || 'value';
                        return `${d.data.value || d.value || 0} ${label}`;
                    }
                    function truncateLabel(text, max = 22) {
                        if (!text) return '';
                        return text.length > max ? text.slice(0, max - 1) + '…' : text;
                    }
                    function normalizeSunburstLayout(node) {
                        const copy = { ...node };
                        const children = Array.isArray(node.children) ? node.children : [];
                        if (!children.length) {
                            copy.layoutValue = sunburstLayoutMetric(node.value);
                            return copy;
                        }

                        const target = sunburstLayoutMetric(node.value);
                        copy.layoutValue = 0;
                        copy.children = children.map(child => normalizeSunburstLayout(child));
                        const childTotal = copy.children.reduce((sum, child) => sum + sunburstLayoutTotal(child), 0) || 1;
                        const scale = target / childTotal;
                        copy.children = copy.children.map(child => scaleSunburstLayout(child, scale));
                        return copy;
                    }

                    function sunburstLayoutMetric(value) {
                        const n = Math.max(Number(value) || 1, 1);
                        return Math.pow(n, 0.7);
                    }

                    function sunburstLayoutTotal(node) {
                        const children = Array.isArray(node.children) ? node.children : [];
                        if (!children.length) return Math.max(Number(node.layoutValue) || 1, 1);
                        return children.reduce((sum, child) => sum + sunburstLayoutTotal(child), 0);
                    }

                    function scaleSunburstLayout(node, scale) {
                        const copy = { ...node };
                        const children = Array.isArray(node.children) ? node.children : [];
                        if (!children.length) {
                            copy.layoutValue = Math.max((Number(node.layoutValue) || 1) * scale, 0.001);
                            return copy;
                        }
                        copy.layoutValue = 0;
                        copy.children = children.map(child => scaleSunburstLayout(child, scale));
                        return copy;
                    }

                    const path = svg.append('g').selectAll('path')
                        .data(root.descendants().slice(1))
                        .join('path')
                        .attr('fill', d => { let n = d; while (n.depth > 1) n = n.parent; return color(n.data.name); })
                        .attr('fill-opacity', d => arcVisible(d.current) ? (d.children ? 0.65 : 0.42) : 0)
                        .attr('pointer-events', d => arcVisible(d.current) ? 'auto' : 'none')
                        .attr('d', d => arc(d.current))
                        .attr('stroke', '#111827')
                        .attr('stroke-width', 0.8)
                        .style('cursor', 'pointer')
                        .on('click', (event, d) => {
                            handlers.onSelect && handlers.onSelect(d.data);
                        })
                        .on('dblclick', (event, d) => {
                            event.preventDefault();
                            if (d.data.term) handlers.onFocus && handlers.onFocus(d.data);
                        });

                    path.append('title').text(d =>
                        `${d.ancestors().map(n => n.data.name).reverse().join(' › ')}\n${nodeValueText(d)}`);

                    svg.append('g')
                        .attr('pointer-events', 'none')
                        .attr('text-anchor', 'middle')
                        .style('user-select', 'none')
                        .selectAll('text')
                        .data(root.descendants().slice(1))
                        .join('text')
                        .attr('dy', '0.35em')
                        .attr('fill', '#fff')
                        .attr('fill-opacity', d => +labelVisible(d.current))
                        .attr('transform', d => labelTransform(d.current))
                        .text(d => truncateLabel(d.data.name));

                    // Center: back indicator when inside a drill-down (breadcrumb.length > 1)
                    const centerGroup = svg.append('g').style('cursor', 'pointer');
                    centerGroup.append('circle')
                        .attr('r', ringWidth * 0.9)
                        .attr('fill', handlers.canBack ? '#243b53' : '#1a1a2e')
                        .attr('stroke', handlers.canBack ? '#5b8def' : '#444')
                        .attr('stroke-width', 1);
                    centerGroup.append('text')
                        .attr('text-anchor', 'middle').attr('dy', '0.35em')
                        .attr('fill', '#aaa').style('font-size', '10px')
                        .text(handlers.canBack ? 'Back' : truncateLabel(hierarchyData.name || 'root', 18));
                    centerGroup.on('click', () => {
                        if (handlers.canBack) handlers.onBack && handlers.onBack();
                    });
                }

                // --- Notes state ---
                const notes = ref([]);
                const notesLoading = ref(true);
                const artifacts = ref([]);
                const artifactsLoading = ref(true);
                const artifactLegacyCount = ref(0);
                const artifactMigrationBusy = ref(false);
                const artifactMigrationMessage = ref('');
                const artifactMigrationItems = ref([]);
                // Every note in the store, any subfolder — `notes` only holds the browsed folder,
                // which cannot answer "does [[X]] exist?" for notes filed under e.g. Concepts/.
                const allNotes = ref([]);
                const knownNoteNames = computed(() =>
                    new Set(allNotes.value.map(n => String(n.name || '').toLowerCase()))
                );
                const noteForm = ref({ open: false, name: '', content: '', editing: false });
                const notePublication = ref({
                    open: false,
                    busy: false,
                    note: null,
                    outputDir: 'published/notes',
                    status: null,
                    result: null,
                    message: ''
                });
                const noteWatch = ref({ root: '', dirtyCount: 0, dirty: [], running: false, lastScanAt: '' });
                const noteWatchBusy = ref(false);
                const noteWatchReportOpen = ref(false);
                let noteWatchPollTimer = null;
                let noteWatchDirtySignature = '';
                const notePublishAllBusy = ref(false);
                const noteTags = ref([]);
                const selectedNoteTag = ref('');
                const noteRelations = ref({
                    open: false,
                    loading: false,
                    name: '',
                    path: '',
                    detail: null,
                    metadataText: '',
                    wikiLinksText: '',
                    wikiLinkInput: '',
                    sourceLinks: [],
                    sourceForm: { sourceId: '', relationType: 'references' },
                    identity: null,
                    permalinkForm: { slug: '', reason: '', preview: null },
                    permalinkBusy: false,
                    message: ''
                });

                // --- Folder browser state ---
                const centerTab = ref('chat');          // 'chat' | 'folders' | 'bounds'
                const folders = ref([]);                 // [{ name, path, count }]
                const foldersLoading = ref(false);
                const currentFolderPath = ref('');       // active notes subfolder ('' = root)
                const folderParent = ref(null);          // parent path ('' = root) or null at root
                const newFolderName = ref('');
                // Breadcrumb segments with cumulative paths, e.g. a/b/c -> [{name:a,path:a},...]
                const folderCrumbs = computed(() => {
                    if (!currentFolderPath.value) return [];
                    const parts = currentFolderPath.value.split('/');
                    return parts.map((name, i) => ({ name, path: parts.slice(0, i + 1).join('/') }));
                });
                const currentFolderName = computed(() => {
                    const p = currentFolderPath.value;
                    return p ? p.substring(p.lastIndexOf('/') + 1) : '';
                });

                // --- Bounds browser state ---
                const bounds = ref([]);
                const boundsLoading = ref(false);
                const selectedBound = ref(null);
                const boundResources = ref([]);
                const boundResourcesLoading = ref(false);
                const boundPath = ref('');
                const boundParent = ref(null);
                const boundUpdates = ref([]);
                const boundImporting = ref('');
                const boundMessage = ref('');
                const boundCrumbs = computed(() => {
                    if (!boundPath.value) return [];
                    const parts = boundPath.value.split('/');
                    return parts.map((name, i) => ({ name, path: parts.slice(0, i + 1).join('/') }));
                });

                // --- Folder tree state ---
                // openFolders: { dirName: boolean } — undefined means open (default)
                const openFolders = ref({});
                const draggingOverFolder = ref(null);

                function extractDir(path) {
                    if (!path) return '';
                    const normalized = path.replace(/\\/g, '/');
                    const lastSlash = normalized.lastIndexOf('/');
                    if (lastSlash < 0) return '';
                    let dir = normalized.substring(0, lastSlash);
                    dir = dir.replace(/^\.\//, '').replace(/^\//, '');
                    return dir;
                }

                // Builds sorted tree [{dir, docs, isPdfGroup?}] from the filtered flat list.
                // PDF chunk groups (by parentId) appear as virtual folders before filesystem dirs.
                const sourceTree = computed(() => {
                    const singles = [];
                    const pdfGroups = new Map(); // parentId -> {groupName, docs[]}

                    filteredSources.value.forEach(doc => {
                        if (doc.parentId) {
                            if (!pdfGroups.has(doc.parentId))
                                pdfGroups.set(doc.parentId, { groupName: doc.groupName || doc.parentId, docs: [] });
                            pdfGroups.get(doc.parentId).docs.push(doc);
                        } else {
                            singles.push(doc);
                        }
                    });

                    // Sort each PDF group by chunkIndex
                    pdfGroups.forEach(g => {
                        g.docs.sort((a, b) => (parseInt(a.chunkIndex) || 0) - (parseInt(b.chunkIndex) || 0));
                    });

                    // Filesystem dirs from non-chunk singles
                    const dirMap = new Map();
                    singles.forEach(doc => {
                        const dir = extractDir(doc.path);
                        if (!dirMap.has(dir)) dirMap.set(dir, []);
                        dirMap.get(dir).push(doc);
                    });

                    const fsGroups = [...dirMap.entries()]
                        .sort(([a], [b]) => a === '' ? -1 : b === '' ? 1 : a.localeCompare(b))
                        .map(([dir, docs]) => ({ dir, docs, isPdfGroup: false }));

                    const pdfTreeEntries = [...pdfGroups.entries()]
                        .sort(([, a], [, b]) => a.groupName.localeCompare(b.groupName))
                        .map(([parentId, g]) => ({ dir: g.groupName, docs: g.docs, isPdfGroup: true, parentId }));

                    // PDF virtual folders first, then filesystem groups
                    return [...pdfTreeEntries, ...fsGroups];
                });

                function isFolderOpen(dir) {
                    if (sourceFilter.value.trim()) return true; // auto-expand when filtering
                    return openFolders.value[dir] !== false;    // default: open
                }

                function toggleFolder(dir) {
                    openFolders.value = { ...openFolders.value, [dir]: !isFolderOpen(dir) };
                }

                function closeAllFolders() {
                    const closed = {};
                    sourceTree.value.forEach(g => { if (g.dir) closed[g.dir] = false; });
                    openFolders.value = closed;
                }

                function handleFolderDrop(dir, event) {
                    event.preventDefault();
                    event.stopPropagation();
                    draggingOverFolder.value = null;
                    const file = event.dataTransfer.files[0];
                    if (file) uploadDocument(file, dir);
                }

                function handleFolderDragOver(dir, event) {
                    event.preventDefault();
                    event.stopPropagation();
                    draggingOverFolder.value = dir;
                    draggingOver.value = false;
                }

                function handleFolderDragLeave(dir, event) {
                    if (!event.currentTarget.contains(event.relatedTarget))
                        draggingOverFolder.value = null;
                }

                // --- term index loaded at startup: { term(stemmed) → [docId, ...] }
                const termIndex = ref({});
                const termIndexLoaded = ref(false);
                const termMatchCount = ref(0);
                const suggestedQuestions = computed(() => {
                    const entries = Object.entries(termIndex.value || {})
                        .map(([term, docIds]) => ({ term, count: Array.isArray(docIds) ? docIds.length : 0 }))
                        .filter(({ term, count }) => count > 0 && term.length >= 4 && !/^\d+$/.test(term))
                        .sort((a, b) => b.count - a.count || a.term.localeCompare(b.term))
                        .slice(0, 6);

                    const terms = entries.map(e => humanizeTerm(e.term));
                    const questions = [];
                    if (terms[0]) questions.push(`What are the key points about ${terms[0]}?`);
                    if (terms[1]) questions.push(`How is ${terms[1]} connected to the other sources?`);
                    if (terms[2]) questions.push(`Summarize the documents that mention ${terms[2]}.`);
                    if (terms[3]) questions.push(`Compare ${terms[0]} and ${terms[3]} across the sources.`);
                    return questions.slice(0, 4);
                });

                function humanizeTerm(term) {
                    return String(term || '')
                        .replace(/[_-]+/g, ' ')
                        .trim()
                        .replace(/\s+/g, ' ')
                        .replace(/\b\w/g, ch => ch.toUpperCase());
                }

                async function askSuggestedQuestion(question) {
                    if (!question || isStreaming.value) return;
                    userInput.value = question;
                    await nextTick();
                    autoResize();
                    await sendMessage();
                }

                const filteredSources = computed(() => {
                    const q = sourceFilter.value.trim().toLowerCase();
                    if (!q) { termMatchCount.value = 0; luceneSearchActive.value = false; return sources.value; }

                    // When Lucene hybrid search results are ready, use ranked order
                    if (luceneSearchActive.value && luceneResults.value.length > 0) {
                        const ranked = luceneResults.value
                            .map(r => sources.value.find(s => s.id === r.documentId))
                            .filter(Boolean);
                        const rankedIds = new Set(ranked.map(s => s.id));
                        // Append any filename-matched docs not in Lucene results
                        const extra = sources.value.filter(d =>
                            !rankedIds.has(d.id) && (
                                d.filename.toLowerCase().includes(q) ||
                                d.path.toLowerCase().includes(q)
                            )
                        );
                        termMatchCount.value = ranked.length;
                        return [...ranked, ...extra];
                    }

                    // Fallback: bidirectional term index match + filename filter
                    const termMatchedIds = new Set();
                    Object.entries(termIndex.value).forEach(([term, docIds]) => {
                        if (term.includes(q) || q.includes(term))
                            docIds.forEach(id => termMatchedIds.add(id));
                    });
                    const result = sources.value.filter(d =>
                        d.filename.toLowerCase().includes(q) ||
                        d.path.toLowerCase().includes(q) ||
                        termMatchedIds.has(d.id)
                    );
                    termMatchCount.value = result.filter(d =>
                        !d.filename.toLowerCase().includes(q) &&
                        !d.path.toLowerCase().includes(q)
                    ).length;
                    return result;
                });

                let searchDebounceTimer = null;
                watch(sourceFilter, (newVal) => {
                    const q = newVal.trim();
                    luceneSearchActive.value = false;
                    clearTimeout(searchDebounceTimer);
                    if (q.length < 3) { luceneResults.value = []; return; }
                    searchDebounceTimer = setTimeout(async () => {
                        try {
                            const res = await fetch('/search?q=' + encodeURIComponent(q));
                            if (!res.ok) return;
                            luceneResults.value = await res.json();
                            luceneSearchActive.value = luceneResults.value.length > 0;
                        } catch (e) {
                            console.warn('Lucene search unavailable:', e.message);
                        }
                    }, 350);
                });

                async function loadTermIndex() {
                    try {
                        const res = await fetch('/mindmap/terms');
                        if (!res.ok) return;
                        const data = await res.json();
                        termIndex.value = data.terms || {};
                        termIndexLoaded.value = Object.keys(termIndex.value).length > 0;
                    } catch (e) {
                        console.warn('Term index not available:', e.message);
                    }
                }

                async function refreshSources() {
                    sourcesLoading.value = true;
                    try {
                        const res = await fetch('/documents');
                        sources.value = await res.json();
                    } catch (e) {
                        sources.value = [];
                    } finally {
                        sourcesLoading.value = false;
                    }
                }

                function docExt(doc) {
                    const name = doc.filename || doc.path || '';
                    const dot = name.lastIndexOf('.');
                    return dot >= 0 ? name.slice(dot + 1).toLowerCase() : '?';
                }

                let clickTimer = null;

                function insertDocRef(doc) {
                    if (clickTimer) clearTimeout(clickTimer);
                    clickTimer = setTimeout(() => {
                        clickTimer = null;
                        const mention = `[${doc.filename}] `;
                        userInput.value += mention;
                        nextTick(() => inputArea.value?.focus());
                    }, 220);
                }

                // --- Notes functions ---
                async function loadNotes() {
                    notesLoading.value = true;
                    try {
                        const tagQuery = selectedNoteTag.value
                            ? `?tag=${encodeURIComponent(selectedNoteTag.value)}`
                            : '';
                        const res = await fetch('/notes' + tagQuery, { cache: 'no-store' });
                        notes.value = res.ok ? await res.json() : [];
                    } catch (e) {
                        notes.value = [];
                    } finally {
                        notesLoading.value = false;
                    }
                    // Keep [[ ]] resolution in sync before callers consider the refresh complete.
                    await loadAllNotes();
                    const selectionCleared = await loadNoteTags();
                    if (selectionCleared) {
                        const res = await fetch('/notes', { cache: 'no-store' });
                        notes.value = res.ok ? await res.json() : [];
                    }
                }

                async function loadArtifacts() {
                    artifactsLoading.value = true;
                    try {
                        const res = await fetch('/artifacts', { cache: 'no-store' });
                        artifacts.value = res.ok ? await res.json() : [];
                        const migrationRes = await fetch('/artifacts/migration', {
                            method: 'POST', headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ apply: false })
                        });
                        const migration = migrationRes.ok ? await migrationRes.json() : null;
                        artifactLegacyCount.value = migration?.total || 0;
                        artifactMigrationItems.value = migration?.items || [];
                    } catch (e) {
                        artifacts.value = [];
                        artifactLegacyCount.value = 0;
                        artifactMigrationItems.value = [];
                    } finally {
                        artifactsLoading.value = false;
                    }
                }

                function artifactIcon(kind) {
                    return ({ flashcards: '📇', quiz: '📋', think: '💭', report: '📝', canvas: '🕸️' })[kind] || '🧩';
                }

                function artifactUrl(artifact) {
                    const ref = encodeURIComponent(artifact.ref || artifact.path || artifact.name);
                    if (artifact.kind === 'flashcards') return `/flashcards?note=${ref}&mode=play`;
                    if (artifact.kind === 'quiz') return `/quiz?note=${ref}&mode=play`;
                    if (artifact.kind === 'think') return `/think?note=${ref}`;
                    if (artifact.kind === 'report') return `/report?note=${ref}&mode=play`;
                    if (artifact.kind === 'canvas') return `/canvas?ref=${ref}&window=1`;
                    return '';
                }

                function openArtifact(artifact) {
                    const url = artifactUrl(artifact);
                    if (url) window.open(url, '_blank');
                    else alert(`No editor is available yet for ${artifact.kind}.`);
                }

                async function deleteArtifact(artifact) {
                    if (!confirm(`Eliminare l'artefatto "${artifact.name}"?`)) return;
                    try {
                        const res = await fetch(`/artifacts/content?ref=${encodeURIComponent(artifact.ref)}`, { method: 'DELETE' });
                        if (!res.ok) { const err = await res.json().catch(() => ({})); alert(err.error || 'Delete failed'); return; }
                        await loadArtifacts();
                    } catch (e) {
                        alert('Delete error: ' + e.message);
                    }
                }

                async function renameArtifact(artifact) {
                    const newName = prompt('Nuovo nome artefatto:', artifact.name);
                    if (newName === null || !newName.trim() || newName.trim() === artifact.name) return;
                    try {
                        const res = await fetch(`/artifacts/rename?ref=${encodeURIComponent(artifact.ref)}`, {
                            method: 'POST', headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ newName: newName.trim(), expectedRevision: artifact.revision })
                        });
                        if (!res.ok) { const err = await res.json().catch(() => ({})); alert(err.error || 'Rename failed'); return; }
                        await loadArtifacts();
                    } catch (e) {
                        alert('Rename error: ' + e.message);
                    }
                }

                async function duplicateArtifact(artifact) {
                    const copyName = prompt('Nome della copia:', `${artifact.name}-copy`);
                    if (copyName === null || !copyName.trim()) return;
                    try {
                        const contentRes = await fetch(`/artifacts/content?ref=${encodeURIComponent(artifact.ref)}`, { cache: 'no-store' });
                        const content = await contentRes.json();
                        if (!contentRes.ok) throw new Error(content.error || `HTTP ${contentRes.status}`);
                        const res = await fetch('/artifacts', {
                            method: 'POST', headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ name: copyName.trim(), type: artifact.kind, content: content.content })
                        });
                        if (!res.ok) { const err = await res.json().catch(() => ({})); throw new Error(err.error || `HTTP ${res.status}`); }
                        await loadArtifacts();
                    } catch (e) {
                        alert('Duplicate error: ' + e.message);
                    }
                }

                async function migrateArtifacts(apply) {
                    if (apply && !confirm('Spostare gli artefatti legacy dalla cartella Notes alla cartella Artifacts?')) return;
                    artifactMigrationBusy.value = true;
                    artifactMigrationMessage.value = '';
                    try {
                        const res = await fetch('/artifacts/migration', {
                            method: 'POST', headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ apply })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        artifactMigrationMessage.value = apply
                            ? `${data.migrated} migrated, ${data.conflicts} conflicts`
                            : `${data.ready} ready, ${data.conflicts} conflicts`;
                        artifactMigrationItems.value = data.items || [];
                        await Promise.all([loadNotes(), loadArtifacts()]);
                    } catch (e) {
                        artifactMigrationMessage.value = 'Migration error: ' + e.message;
                    } finally {
                        artifactMigrationBusy.value = false;
                    }
                }

                async function loadNoteTags() {
                    try {
                        const res = await fetch('/notes/tags', { cache: 'no-store' });
                        noteTags.value = res.ok ? await res.json() : [];
                        if (selectedNoteTag.value && !noteTags.value.some(t => t.name === selectedNoteTag.value)) {
                            selectedNoteTag.value = '';
                            return true;
                        }
                    } catch (e) {
                        noteTags.value = [];
                    }
                    return false;
                }

                async function selectNoteTag(tag) {
                    selectedNoteTag.value = tag || '';
                    await loadNotes();
                }

                function resetNoteTagFilter() {
                    selectedNoteTag.value = '';
                }

                async function loadAllNotes() {
                    try {
                        const res = await fetch('/notes?recursive=true', { cache: 'no-store' });
                        allNotes.value = res.ok ? await res.json() : [];
                    } catch (e) {
                        allNotes.value = [];
                    }
                }

                function resolveNoteByName(name) {
                    const sourceRef = currentFolderPath.value ? `${currentFolderPath.value}/_current` : '_current';
                    return window.KubiWikiLinks.resolveNote(name, allNotes.value, sourceRef);
                }

                function findNoteByName(name) {
                    const resolution = resolveNoteByName(name);
                    return resolution.status === 'found' ? resolution.note : null;
                }

                /**
                 * Click on a [[wiki link]]: open the note if it exists, otherwise go create it.
                 * The cached list may be stale (note added from another tab, or just now), so a
                 * miss is re-checked against the server before we offer to create a duplicate.
                 */
                async function openWikiLink(target) {
                    let resolution = resolveNoteByName(target);
                    if (resolution.status !== 'found') {
                        await loadAllNotes();
                        resolution = resolveNoteByName(target);
                    }
                    if (resolution.status === 'ambiguous') {
                        alert(`Il link [[${target}]] è ambiguo. Specifica il percorso della nota.`);
                        return;
                    }
                    if (resolution.status === 'found') {
                        await viewNote(resolution.note);   // routes by kind: flashcards/quiz/think get their player
                        return;
                    }
                    window.location.href = window.KubiWikiLinks.newNoteHref(target);
                }

                /**
                 * Same jump, from inside a modal (concept backlinks, evidence backlinks, note
                 * relations). The panels must go first or the note opens underneath them.
                 * Closing an already-closed panel is a no-op, so one handler serves them all —
                 * which also keeps the three templates textually identical.
                 */
                function openNoteFromPanel(noteName) {
                    conceptsPanelOpen.value = false;
                    evidencePanelOpen.value = false;
                    closeNoteRelations();
                    openWikiLink(noteName);
                }

                function dirtySignature(status) {
                    return (status?.dirty || [])
                        .map(item => `${item.path}:${item.lastModified}`)
                        .sort()
                        .join('|');
                }

                function openNoteWatchReport() {
                    noteWatchReportOpen.value = true;
                }

                function noteWatchChangeLabel(change) {
                    return ({ added: 'Aggiunta', modified: 'Modificata', removed: 'Rimossa' })[change] || 'Modificata';
                }

                function formatNoteWatchTime(value) {
                    if (!value) return '';
                    const date = new Date(value);
                    return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
                }

                async function reloadFilesystemNotes() {
                    await Promise.all([loadNotes(), loadFolders()]);
                }

                async function loadNoteWatchStatus(reloadOnChange = false) {
                    try {
                        const res = await fetch('/notes/watch/status', { cache: 'no-store' });
                        if (res.ok) {
                            const status = await res.json();
                            const nextSignature = dirtySignature(status);
                            const changed = nextSignature !== noteWatchDirtySignature;
                            noteWatch.value = status;
                            noteWatchDirtySignature = nextSignature;
                            if (reloadOnChange && changed) await reloadFilesystemNotes();
                        }
                    } catch (e) {
                        console.warn('Note watch status unavailable:', e.message);
                    }
                }

                function startNoteWatchPolling() {
                    if (noteWatchPollTimer !== null) return;
                    noteWatchPollTimer = window.setInterval(() => loadNoteWatchStatus(true), 2000);
                }

                function stopNoteWatchPolling() {
                    if (noteWatchPollTimer === null) return;
                    window.clearInterval(noteWatchPollTimer);
                    noteWatchPollTimer = null;
                }

                async function startNoteWatch() {
                    noteWatchBusy.value = true;
                    try {
                        const res = await fetch('/notes/watch/start', { method: 'POST' });
                        if (res.ok) {
                            noteWatch.value = await res.json();
                            noteWatchDirtySignature = dirtySignature(noteWatch.value);
                            startNoteWatchPolling();
                        }
                    } finally {
                        noteWatchBusy.value = false;
                    }
                }

                async function stopNoteWatch() {
                    noteWatchBusy.value = true;
                    try {
                        const res = await fetch('/notes/watch/stop', { method: 'POST' });
                        if (res.ok) noteWatch.value = await res.json();
                        stopNoteWatchPolling();
                    } finally {
                        noteWatchBusy.value = false;
                    }
                }

                async function scanNoteWatch() {
                    noteWatchBusy.value = true;
                    try {
                        const res = await fetch('/notes/watch/scan', { method: 'POST' });
                        if (res.ok) {
                            noteWatch.value = await res.json();
                            noteWatchDirtySignature = dirtySignature(noteWatch.value);
                            await reloadFilesystemNotes();
                        }
                    } finally {
                        noteWatchBusy.value = false;
                    }
                }

                async function refreshChangedNotes() {
                    noteWatchBusy.value = true;
                    try {
                        const res = await governedFetch('/notes/watch/refresh', { method: 'POST' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        await loadNoteWatchStatus();
                        await loadNotes();
                        await refreshSources();
                        await loadEvidenceStatus();
                    } catch (e) {
                        alert('Refresh error: ' + e.message);
                    } finally {
                        noteWatchBusy.value = false;
                    }
                }

                // --- Folder browser functions ---
                async function loadFolders() {
                    foldersLoading.value = true;
                    try {
                        const res = await fetch('/notes/folders', { cache: 'no-store' });
                        const data = await res.json();
                        folders.value = data.folders || [];
                        currentFolderPath.value = data.current || '';
                        folderParent.value = data.parent;   // '' (root) or null (already root)
                    } catch (e) {
                        folders.value = [];
                    } finally {
                        foldersLoading.value = false;
                    }
                }

                // Set the active notes subfolder server-side, then refresh both panels.
                async function goToFolder(path) {
                    try {
                        selectedNoteTag.value = '';
                        const res = await fetch('/notes/folder', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ path: path || '' })
                        });
                        if (!res.ok) { const err = await res.json().catch(() => ({})); alert(err.error || 'Folder switch failed'); return; }
                        await loadFolders();
                        await loadNotes();
                    } catch (e) {
                        alert('Folder switch error: ' + e.message);
                    }
                }

                function selectFolder(f) { return goToFolder(f.path); }

                async function createFolder() {
                    const name = newFolderName.value.trim();
                    if (!name) return;
                    try {
                        const res = await fetch('/notes/folders', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ name })
                        });
                        if (!res.ok) { const err = await res.json().catch(() => ({})); alert(err.error || 'Create folder failed'); return; }
                        newFolderName.value = '';
                        await loadFolders();
                    } catch (e) {
                        alert('Create folder error: ' + e.message);
                    }
                }

                function openFoldersTab() {
                    centerTab.value = 'folders';
                    loadFolders();
                }

                async function loadBounds() {
                    boundsLoading.value = true;
                    boundMessage.value = '';
                    try {
                        const res = await fetch('/bounds');
                        if (!res.ok) throw new Error(`HTTP ${res.status}`);
                        bounds.value = await res.json();
                        if (selectedBound.value) {
                            selectedBound.value = bounds.value.find(b => b.bound.name === selectedBound.value.bound.name) || null;
                        }
                    } catch (e) {
                        bounds.value = [];
                        boundMessage.value = 'Unable to load bounds: ' + e.message;
                    } finally {
                        boundsLoading.value = false;
                    }
                }

                async function selectBound(item) {
                    selectedBound.value = item;
                    boundPath.value = '';
                    boundResources.value = [];
                    boundUpdates.value = [];
                    boundMessage.value = '';
                    await Promise.all([loadBoundResources(''), loadBoundUpdates()]);
                }

                async function loadBoundResources(path) {
                    if (!selectedBound.value || selectedBound.value.status.availability !== 'LOCAL') return;
                    boundResourcesLoading.value = true;
                    try {
                        const name = encodeURIComponent(selectedBound.value.bound.name);
                        const res = await fetch(`/bounds/${name}/resources?path=${encodeURIComponent(path || '')}`);
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        boundResources.value = data.resources || [];
                        boundPath.value = data.path || '';
                        boundParent.value = data.parent;
                    } catch (e) {
                        boundMessage.value = 'Resource navigation failed: ' + e.message;
                    } finally {
                        boundResourcesLoading.value = false;
                    }
                }

                async function loadBoundUpdates() {
                    if (!selectedBound.value) return;
                    try {
                        const name = encodeURIComponent(selectedBound.value.bound.name);
                        const res = await fetch(`/bounds/${name}/updates`);
                        boundUpdates.value = res.ok ? await res.json() : [];
                    } catch (e) { boundUpdates.value = []; }
                }

                async function importBoundResource(resource) {
                    if (!selectedBound.value) return;
                    boundImporting.value = resource.relativePath;
                    boundMessage.value = '';
                    try {
                        const name = encodeURIComponent(selectedBound.value.bound.name);
                        const res = await governedFetch(`/bounds/${name}/sources`, {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ path: resource.relativePath, recursive: resource.directory })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        boundMessage.value = `Imported ${data.imported}, updated ${data.updated}, skipped ${data.skipped}, failed ${data.failed}`;
                        if (data.errors && data.errors.length) boundMessage.value += ` — ${data.errors.join('; ')}`;
                        await refreshSources();
                    } catch (e) {
                        boundMessage.value = 'Import failed: ' + e.message;
                    } finally {
                        boundImporting.value = '';
                    }
                }

                async function addSelectedBoundAsSource() {
                    if (!selectedBound.value || selectedBound.value.status.availability !== 'LOCAL') return;
                    boundImporting.value = '__bound_root__';
                    boundMessage.value = '';
                    try {
                        const name = encodeURIComponent(selectedBound.value.bound.name);
                        const res = await governedFetch(`/bounds/${name}/sources`, {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ path: '', recursive: true })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        boundMessage.value = `Source saved: imported ${data.imported}, updated ${data.updated}, skipped ${data.skipped}, failed ${data.failed}`;
                        if (data.errors && data.errors.length) boundMessage.value += ` — ${data.errors.join('; ')}`;
                        await refreshSources();
                    } catch (e) {
                        boundMessage.value = 'Import failed: ' + e.message;
                    } finally {
                        boundImporting.value = '';
                    }
                }

                function openBoundsTab() {
                    centerTab.value = 'bounds';
                    loadBounds();
                }

                function prettyJson(value) { return JSON.stringify(value || {}, null, 2); }

                function openFlashcards() {
                    window.location.href = '/flashcards?note=__new__';
                }

                function openQuiz() {
                    window.location.href = '/quiz?note=__new__';
                }

                function openThink() {
                    window.location.href = '/think?note=__new__';
                }

                function openReport() {
                    window.location.href = '/report?note=__new__';
                }

                function openCanvas() {
                    window.open('/canvas?window=1', '_blank');
                }

                function openNoteForm(note) {
                    if (note && note.kind === 'flashcards') {
                        window.location.href = `/flashcards?note=${encodeURIComponent(note.name)}`;
                        return;
                    }
                    if (note && note.kind === 'quiz') {
                        window.location.href = `/quiz?note=${encodeURIComponent(note.name)}`;
                        return;
                    }
                    if (note && note.kind === 'think') {
                        window.location.href = `/think?note=${encodeURIComponent(note.name)}`;
                        return;
                    }
                    if (note && note.kind === 'report') {
                        window.location.href = `/report?note=${encodeURIComponent(note.name)}`;
                        return;
                    }
                    if (note) {
                        window.location.href = `/markboard?note=${encodeURIComponent(note.ref || String(note.path || note.name).replace(/\.md$/i, ''))}`;
                    } else {
                        const params = new URLSearchParams({ note: '__new__' });
                        if (currentFolderPath.value) params.set('folder', currentFolderPath.value);
                        window.location.href = `/markboard?${params.toString()}`;
                    }
                }

                async function openNotePublication(note) {
                    notePublication.value = {
                        open: true,
                        busy: true,
                        note,
                        outputDir: 'published/notes',
                        status: null,
                        result: null,
                        message: ''
                    };
                    try {
                        const pathQuery = note.path ? `?path=${encodeURIComponent(note.path)}` : '';
                        const res = await fetch(`/notes/${encodeURIComponent(note.name)}/publication${pathQuery}`, { cache: 'no-store' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        notePublication.value.status = data;
                        notePublication.value.outputDir = data.outputDir || 'published/notes';
                    } catch (e) {
                        notePublication.value.message = 'Publication status error: ' + e.message;
                    } finally {
                        notePublication.value.busy = false;
                    }
                }

                function closeNotePublication() {
                    notePublication.value.open = false;
                }

                async function publishSelectedNote(overwriteHash = null) {
                    const state = notePublication.value;
                    const note = state.note;
                    if (!note || state.busy) return;
                    state.busy = true;
                    state.message = '';
                    state.result = null;
                    try {
                        const pathQuery = note.path ? `?path=${encodeURIComponent(note.path)}` : '';
                        const res = await fetch(`/notes/${encodeURIComponent(note.name)}/publication/publish${pathQuery}`, {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ outputDir: state.outputDir.trim() || 'published/notes', overwriteHash })
                        });
                        const data = await res.json();
                        if (res.status === 409 && data.targetHash) {
                            if (confirm(`${data.error}\n\nOverwrite the published file?`)) {
                                state.busy = false;
                                await publishSelectedNote(data.targetHash);
                            } else {
                                state.message = 'Publication cancelled.';
                            }
                            return;
                        }
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        state.result = data;
                        state.message = data.changed ? `Published to ${data.targetPath}` : `Already up to date: ${data.targetPath}`;
                        state.status = { ...(state.status || {}), outputDir: state.outputDir, targetPath: data.targetPath, exists: true, tracked: true, modified: false, targetHash: data.outputHash };
                    } catch (e) {
                        state.message = 'Publication error: ' + e.message;
                    } finally {
                        state.busy = false;
                    }
                }

                async function publishAllNotes(overwriteHashes = {}, confirmed = false) {
                    if (notePublishAllBusy.value) return;
                    if (!confirmed && !confirm('Publish all notebook notes as GitHub Markdown?')) return;
                    notePublishAllBusy.value = true;
                    try {
                        const res = await fetch('/notes/publication/publish-all', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ outputDir: '', overwriteHashes })
                        });
                        const data = await res.json();
                        if (res.status === 409 && data.targetPath && data.targetHash) {
                            if (confirm(`${data.error}\n\nOverwrite this file and continue publishing all notes?`)) {
                                notePublishAllBusy.value = false;
                                await publishAllNotes({ ...overwriteHashes, [data.targetPath]: data.targetHash }, true);
                            }
                            return;
                        }
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        alert(
                            `Published ${data.total} notes to ${data.outputDir}.\n` +
                            `${data.published} changed, ${data.unchanged} unchanged, ${data.warningCount} warnings.`
                        );
                    } catch (e) {
                        alert('Publish all notes error: ' + e.message);
                    } finally {
                        notePublishAllBusy.value = false;
                    }
                }

                function metadataToText(metadata) {
                    return Object.entries(metadata || {})
                        .sort(([a], [b]) => a.localeCompare(b))
                        .map(([key, value]) => `${key}=${value}`)
                        .join('\n');
                }

                function parseMetadataText(text) {
                    const result = {};
                    String(text || '').split('\n').forEach(line => {
                        const trimmed = line.trim();
                        if (!trimmed || trimmed.startsWith('#')) return;
                        const idx = trimmed.indexOf('=');
                        if (idx <= 0) return;
                        const key = trimmed.slice(0, idx).trim();
                        const value = trimmed.slice(idx + 1).trim();
                        if (key) result[key] = value;
                    });
                    return result;
                }

                function linksToText(links) {
                    return (links || []).join('\n');
                }

                function parseLinksText(text) {
                    return String(text || '').split('\n')
                        .map(line => line.trim().replace(/^\[\[/, '').replace(/]]$/, ''))
                        .filter(Boolean)
                        .filter((value, index, all) => all.findIndex(v => v.toLowerCase() === value.toLowerCase()) === index);
                }

                const noteLinkSuggestions = computed(() => {
                    const counts = new Map();
                    allNotes.value.forEach(note => {
                        const key = String(note.name || '').toLowerCase();
                        counts.set(key, (counts.get(key) || 0) + 1);
                    });
                    const currentRef = String(noteRelations.value.path || '').replace(/\.md$/i, '');
                    return allNotes.value
                        .filter(note => (note.ref || '') !== currentRef)
                        .map(note => counts.get(String(note.name || '').toLowerCase()) === 1
                            ? note.name
                            : (note.ref || String(note.path || '').replace(/\.md$/i, '') || note.name))
                        .sort((a, b) => a.localeCompare(b));
                });

                const outgoingNoteLinks = computed(() => {
                    const detail = noteRelations.value.detail;
                    if (!detail) return [];
                    if (Array.isArray(detail.outgoingLinks)) return detail.outgoingLinks;
                    // Compatibility with servers predating the structured outgoingLinks field.
                    return (detail.wikiLinks || []).map(target => ({
                        target, noteRef: null, status: 'missing', occurrences: 1, formats: ['wiki'], candidateRefs: []
                    }));
                });

                function outgoingLinkTitle(link) {
                    if (link.status === 'ambiguous') {
                        const candidates = (link.candidateRefs || []).join(', ');
                        return candidates ? `Ambiguous: ${candidates}` : 'Ambiguous note link';
                    }
                    const formats = (link.formats || []).join(' + ');
                    return `${link.noteRef || link.target}${formats ? ` · ${formats}` : ''}`;
                }

                function openOutgoingLink(link) {
                    if (!link) return;
                    if (link.status === 'ambiguous') {
                        const candidates = (link.candidateRefs || []).join('\n');
                        alert(`Il link [[${link.target}]] è ambiguo.${candidates ? `\n\nUsa uno di questi percorsi:\n${candidates}` : ''}`);
                        return;
                    }
                    openNoteFromPanel(link.noteRef || link.target);
                }

                function addManagedWikiLink() {
                    const value = String(noteRelations.value.wikiLinkInput || '').trim()
                        .replace(/^\[\[/, '').replace(/]]$/, '');
                    if (!value) return;
                    const links = parseLinksText(noteRelations.value.wikiLinksText);
                    if (!links.some(link => link.toLowerCase() === value.toLowerCase())) links.push(value);
                    noteRelations.value.wikiLinksText = links.join('\n');
                    noteRelations.value.wikiLinkInput = '';
                }

                function removeManagedWikiLink(value) {
                    noteRelations.value.wikiLinksText = parseLinksText(noteRelations.value.wikiLinksText)
                        .filter(link => link.toLowerCase() !== String(value).toLowerCase()).join('\n');
                }

                async function openNoteRelations(note) {
                    if (!note) return;
                    noteRelations.value = {
                        ...noteRelations.value,
                        open: true,
                        loading: true,
                        name: note.name,
                        path: note.path || '',
                        wikiLinkInput: '',
                        message: ''
                    };
                    try {
                        const pathQuery = note.path ? `?path=${encodeURIComponent(note.path)}` : '';
                        const res = await fetch(`/notes/${encodeURIComponent(note.name)}/relations${pathQuery}`);
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        noteRelations.value.detail = data;
                        noteRelations.value.metadataText = metadataToText(data.metadata);
                        noteRelations.value.wikiLinksText = linksToText(data.managedWikiLinks || []);
                        noteRelations.value.sourceLinks = [...(data.sourceLinks || [])];
                        if (data.resourceUri) {
                            const identityRes = await fetch('/resources/resolve?uri=' + encodeURIComponent(data.resourceUri), { cache: 'no-store' });
                            noteRelations.value.identity = identityRes.ok ? await identityRes.json() : null;
                        } else {
                            noteRelations.value.identity = null;
                        }
                    } catch (e) {
                        noteRelations.value.message = 'Relations load error: ' + e.message;
                    } finally {
                        noteRelations.value.loading = false;
                    }
                }

                async function previewNotePermalink() {
                    const slug = noteRelations.value.permalinkForm.slug.trim();
                    noteRelations.value.permalinkForm.preview = null;
                    if (!slug) return;
                    try {
                        const res = await fetch('/resources/permalinks/preview?' + new URLSearchParams({ kind: 'page', slug }), { cache: 'no-store' });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        noteRelations.value.permalinkForm.preview = data;
                    } catch (e) {
                        noteRelations.value.message = 'Permalink preview error: ' + e.message;
                    }
                }

                async function promoteNotePermalink() {
                    const identity = noteRelations.value.identity;
                    const form = noteRelations.value.permalinkForm;
                    if (!identity || !form.preview?.available || !form.reason.trim() || noteRelations.value.permalinkBusy) return;
                    if (!confirm(`Publish ${form.preview.permalink}? Published permalinks can never be reassigned.`)) return;
                    noteRelations.value.permalinkBusy = true;
                    try {
                        const statusRes = await fetch('/resources/identity/status', { cache: 'no-store' });
                        const status = await statusRes.json();
                        const res = await fetch('/resources/permalinks', {
                            method: 'POST', headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({
                                resourceUri: identity.uri, slug: form.slug, expectedRevision: status.registryRevision,
                                actor: 'ragboard-user', reason: form.reason,
                                operationId: crypto.randomUUID ? crypto.randomUUID() : `ragboard-${Date.now()}`
                            })
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        const identityRes = await fetch('/resources/resolve?uri=' + encodeURIComponent(identity.uri), { cache: 'no-store' });
                        noteRelations.value.identity = await identityRes.json();
                        noteRelations.value.permalinkForm = { slug: '', reason: '', preview: null };
                        noteRelations.value.message = 'Permalink published; previous aliases remain active.';
                    } catch (e) {
                        noteRelations.value.message = 'Permalink promotion error: ' + e.message;
                    } finally {
                        noteRelations.value.permalinkBusy = false;
                    }
                }

                function closeNoteRelations() {
                    noteRelations.value = {
                        open: false,
                        loading: false,
                        name: '',
                        path: '',
                        detail: null,
                        metadataText: '',
                        wikiLinksText: '',
                        wikiLinkInput: '',
                        sourceLinks: [],
                        sourceForm: { sourceId: '', relationType: 'references' },
                        identity: null,
                        permalinkForm: { slug: '', reason: '', preview: null },
                        permalinkBusy: false,
                        message: ''
                    };
                }

                function addNoteSourceLink() {
                    const sourceId = noteRelations.value.sourceForm.sourceId.trim();
                    if (!sourceId) return;
                    const existing = noteRelations.value.sourceLinks || [];
                    if (existing.some(link => String(link.sourceId).toLowerCase() === sourceId.toLowerCase())) return;
                    const doc = sources.value.find(d => d.id === sourceId || d.filename === sourceId || d.path === sourceId);
                    noteRelations.value.sourceLinks = [
                        ...existing,
                        {
                            sourceId: doc?.id || sourceId,
                            relationType: noteRelations.value.sourceForm.relationType || 'references',
                            sourceTitle: doc?.filename || sourceId,
                            sourcePath: doc?.path || sourceId
                        }
                    ];
                    noteRelations.value.sourceForm = { sourceId: '', relationType: 'references' };
                }

                function removeNoteSourceLink(sourceId) {
                    noteRelations.value.sourceLinks = (noteRelations.value.sourceLinks || [])
                        .filter(link => link.sourceId !== sourceId);
                }

                async function saveNoteRelations() {
                    if (!noteRelations.value.name) return;
                    noteRelations.value.message = '';
                    try {
                        const body = {
                            metadata: parseMetadataText(noteRelations.value.metadataText),
                            wikiLinks: parseLinksText(noteRelations.value.wikiLinksText),
                            sourceLinks: noteRelations.value.sourceLinks || []
                        };
                        const pathQuery = noteRelations.value.path ? `?path=${encodeURIComponent(noteRelations.value.path)}` : '';
                        const res = await fetch(`/notes/${encodeURIComponent(noteRelations.value.name)}/relations${pathQuery}`, {
                            method: 'PUT',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify(body)
                        });
                        const data = await res.json();
                        if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
                        noteRelations.value.detail = data;
                        noteRelations.value.metadataText = metadataToText(data.metadata);
                        noteRelations.value.wikiLinksText = linksToText(data.managedWikiLinks);
                        noteRelations.value.sourceLinks = [...(data.sourceLinks || [])];
                        noteRelations.value.message = 'Relations saved.';
                        await loadNotes();
                    } catch (e) {
                        noteRelations.value.message = 'Relations save error: ' + e.message;
                    }
                }

                function closeNoteForm() {
                    noteForm.value = { open: false, name: '', content: '', editing: false };
                }

                async function saveNote() {
                    const name = noteForm.value.name.trim();
                    if (!name) { alert('Nome nota richiesto.'); return; }
                    const body = noteForm.value.editing
                        ? { content: noteForm.value.content }
                        : { name, content: noteForm.value.content };
                    const url = noteForm.value.editing ? `/notes/${encodeURIComponent(name)}` : '/notes';
                    const method = noteForm.value.editing ? 'PUT' : 'POST';
                    try {
                        const res = await fetch(url, {
                            method,
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify(body)
                        });
                        if (!res.ok) { const err = await res.json(); alert(err.error || 'Save failed'); return; }
                        closeNoteForm();
                        await loadNotes();
                    } catch (e) {
                        alert('Save error: ' + e.message);
                    }
                }

                async function deleteNote(note) {
                    const name = note.name;
                    if (!confirm(`Eliminare la nota "${name}"?`)) return;
                    try {
                        const pathQuery = note.path ? `?path=${encodeURIComponent(note.path)}` : '';
                        const res = await fetch(`/notes/${encodeURIComponent(name)}${pathQuery}`, { method: 'DELETE' });
                        if (!res.ok) { const err = await res.json(); alert(err.error || 'Delete failed'); return; }
                        if (noteForm.value.open && noteForm.value.name === name) closeNoteForm();
                        await loadNotes();
                    } catch (e) {
                        alert('Delete error: ' + e.message);
                    }
                }

                // A note is pinned when a source document exists for its .md file and is pinned.
                function isNotePinned(note) {
                    const notePath = String(note.path || '').replace(/\\/g, '/');
                    return pinnedDocs.value.some(d =>
                        d.filename === note.name + '.md' &&
                        (!notePath || String(d.path || '').replace(/\\/g, '/').endsWith(notePath))
                    );
                }

                // Toggle pin on a note: POST /notes/{name}/pin indexes it as a source + pins it;
                // unpin reuses the existing source id via DELETE /pinned/{id}.
                async function togglePinNote(note) {
                    try {
                        if (isNotePinned(note)) {
                            const notePath = String(note.path || '').replace(/\\/g, '/');
                            const doc = pinnedDocs.value.find(d =>
                                d.filename === note.name + '.md' &&
                                (!notePath || String(d.path || '').replace(/\\/g, '/').endsWith(notePath))
                            );
                            if (doc) await governedFetch(`/pinned/${encodeURIComponent(doc.id)}`, { method: 'DELETE' });
                        } else {
                            const pathQuery = note.path ? `?path=${encodeURIComponent(note.path)}` : '';
                            const res = await governedFetch(`/notes/${encodeURIComponent(note.name)}/pin${pathQuery}`, { method: 'POST' });
                            if (!res.ok) { const err = await res.json().catch(() => ({})); alert(err.error || 'Pin failed'); return; }
                            await refreshSources();  // a new source document may have been created
                        }
                        await loadPinned();
                    } catch (e) {
                        console.warn('Note pin toggle failed:', e.message);
                    }
                }

                async function viewNote(note) {
                    const noteRef = note.ref || String(note.path || note.name).replace(/\.md$/i, '');
                    viewerDoc.value = {
                        filename: note.filename || note.name + '.md',
                        path: note.authorityPath || `notes/${note.path || noteRef + '.md'}`,
                        kind: 'note',
                        noteRef
                    };
                    viewerHtml.value = '';
                    viewerLoading.value = true;
                    viewerOpen.value = true;
                    try {
                        const pathQuery = note.path ? `?path=${encodeURIComponent(note.path)}` : '';
                        const res = await fetch(`/notes/${encodeURIComponent(note.name)}${pathQuery}`);
                        if (!res.ok) throw new Error(`HTTP ${res.status}`);
                        const data = await res.json();
                        const context = { note: note.name, noteRef, notePath: note.path, title: note.name + '.md' };
                        const { html: noteHtml, diagrams: noteDiagrams } = renderViewerMarkdown(data.content || '', context);
                        viewerHtml.value = noteHtml + backlinksHtml(await loadViewerBacklinks(context));
                        postRenderViewer(noteDiagrams);
                    } catch (e) {
                        viewerHtml.value = `<p style="color:#f28b82;">Note load error: ${e.message}</p>`;
                    } finally {
                        viewerLoading.value = false;
                    }
                }

                async function viewDoc(doc) {
                    if (clickTimer) { clearTimeout(clickTimer); clickTimer = null; }
                    viewerDoc.value = doc;
                    viewerHtml.value = '';
                    viewerEnrichment.value = null;
                    viewerLoading.value = true;
                    viewerOpen.value = true;
                    try {
                        const res = await fetch(`/documents/${encodeURIComponent(doc.id)}`);
                        if (!res.ok) throw new Error(`HTTP ${res.status}`);
                        const data = await res.json();
                        // Enrichment panel — show for all file types
                        if (data.enrichedAt || (data.metadata && data.metadata.enrichedAt)) {
                            const m = data.metadata || data;
                            viewerEnrichment.value = {
                                aiSummary: m.aiSummary || data.aiSummary || '',
                                aiEntities: m.aiEntities || data.aiEntities || '',
                                aiRelations: m.aiRelations || data.aiRelations || '',
                                enrichedAt: (m.enrichedAt || data.enrichedAt || '').substring(0, 16).replace('T', ' '),
                                enrichedByModel: m.enrichedByModel || data.enrichedByModel || ''
                            };
                        }
                        // Content — markdown for md/txt, plain text for everything else
                        const rawExt = (doc.filename || '').split('.').pop().toLowerCase();
                        const ext = rawExt.replace(/\s.*$/, ''); // strip PDF chunk suffix like " [1/5]"
                        if (data.content) {
                            if (['md', 'txt'].includes(ext)) {
                                const context = { doc: doc.id, title: data.filename || doc.filename || '' };
                                const { html: docHtml, diagrams: docDiagrams } = renderViewerMarkdown(data.content, context);
                                viewerHtml.value = docHtml + backlinksHtml(await loadViewerBacklinks(context));
                                postRenderViewer(docDiagrams);
                            } else {
                                const escaped = data.content.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
                                viewerHtml.value = `<pre style="white-space:pre-wrap; word-break:break-word; font-size:12px; line-height:1.6; font-family:inherit;">${escaped}</pre>`;
                            }
                        }
                    } catch (e) {
                        viewerHtml.value = `<p style="color:#f28b82;">Error loading document: ${e.message}</p>`;
                    } finally {
                        viewerLoading.value = false;
                    }
                }

                function openInMarkview() {
                    if (viewerDoc.value.kind === 'note') {
                        window.open(`/markview?note=${encodeURIComponent(viewerDoc.value.noteRef)}`, '_blank');
                    } else if (viewerDoc.value.id) {
                        window.open(`/markview?doc=${encodeURIComponent(viewerDoc.value.id)}`, '_blank');
                    }
                }

                // --- Chat helpers ---

                // Pure function for chat messages — no side effects
                function renderMarkdown(text) {
                    if (!text) return '';
                    return marked.parse(text);
                }

                // The legacy chat stream carries pinned/retrieved documents and the LLM answer
                // in one text field. Split that presentation-only protocol without changing the SSE API.
                // Stable keys let Vue preserve an opened <details> while answer chunks arrive.
                function parseRagResponse(text) {
                    if (!text) return [];

                    const answerMarker = `\n${'='.repeat(60)}\n🤖 RISPOSTA:\n${'='.repeat(60)}\n`;
                    const headerPattern = /(^|\n)(Docs|Pinned)\[(\d+)\]:([^\n]*)\n/g;
                    const headers = Array.from(text.matchAll(headerPattern));
                    if (headers.length === 0) {
                        return [{ key: 'markdown-0', type: 'markdown', content: text }];
                    }

                    const parts = [];
                    let cursor = 0;
                    let markdownIndex = 0;

                    headers.forEach((header, retrievalIndex) => {
                        const blockStart = header.index + header[1].length;
                        const contextStart = header.index + header[0].length;
                        const nextHeader = headers[retrievalIndex + 1];
                        const blockEnd = nextHeader
                            ? nextHeader.index + nextHeader[1].length
                            : text.length;

                        const leading = text.slice(cursor, blockStart);
                        if (leading.trim()) {
                            parts.push({
                                key: `markdown-${markdownIndex++}`,
                                type: 'markdown',
                                content: leading
                            });
                        }

                        const markerIndex = text.indexOf(answerMarker, contextStart);
                        const hasAnswer = markerIndex >= contextStart && markerIndex < blockEnd;
                        const sourceKind = header[2] === 'Pinned' ? 'pinned' : 'retrieved';
                        const rawSources = header[4].trim();
                        const sources = rawSources
                            ? rawSources.split(' - ').map(source => source.trim()).filter(Boolean)
                            : [];

                        parts.push({
                            key: `rag-sources-${sourceKind}-${retrievalIndex}`,
                            type: 'sources',
                            sourceKind,
                            sourceCount: Number.parseInt(header[3], 10) || 0,
                            sources,
                            content: text.slice(contextStart, hasAnswer ? markerIndex : blockEnd).trimEnd()
                        });

                        if (hasAnswer) {
                            const answerStart = markerIndex + answerMarker.length;
                            const answer = text.slice(answerStart, blockEnd);
                            if (answer) {
                                parts.push({
                                    key: `rag-answer-${retrievalIndex}`,
                                    type: 'markdown',
                                    content: answer
                                });
                            }
                        }
                        cursor = blockEnd;
                    });

                    return parts;
                }

                function escapeHtml(value) {
                    return String(value || '')
                        .replace(/&/g, '&amp;')
                        .replace(/</g, '&lt;')
                        .replace(/>/g, '&gt;')
                        .replace(/"/g, '&quot;');
                }

                function preprocessObsidianMarkdown(text, context = {}) {
                    return window.KubiWikiLinks.preprocess(text, context, {
                        notes: allNotes.value,
                        sourceRef: context.noteRef || context.note,
                        // A real href so ctrl/middle-click opens the standalone viewer in a new tab;
                        // a plain left click is intercepted and handled in-app by openWikiLink.
                        href: (target, resolved) => resolved
                            ? `/markview?note=${encodeURIComponent(target)}`
                            : window.KubiWikiLinks.newNoteHref(target)
                    });
                }

                async function loadViewerBacklinks(context = {}) {
                    const noteName = context.note || (context.title || '').replace(/\.md$/i, '');
                    if (!noteName) return [];
                    const url = context.note
                        ? `/notes/${encodeURIComponent(noteName)}/relations${context.notePath ? `?path=${encodeURIComponent(context.notePath)}` : ''}`
                        : `/notes/relations/${encodeURIComponent(noteName)}`;
                    try {
                        const res = await fetch(url);
                        if (!res.ok) return [];
                        const data = await res.json();
                        return data.backlinks || [];
                    } catch (e) {
                        return [];
                    }
                }

                function backlinksHtml(backlinks) {
                    if (!backlinks || backlinks.length === 0) return '';
                    // A backlink was found by walking real files, so its note always exists —
                    // same anchor as an inline [[ ]], picked up by the same delegated handler.
                    const rows = backlinks.map(link => `
                        <li>
                            ${window.KubiWikiLinks.anchor(link.noteRef || link.noteName, link.noteName, {
                                href: `/markview?note=${encodeURIComponent(link.noteRef || link.noteName)}`,
                                title: link.snippet || link.path || link.noteName
                            })}
                            <small>${escapeHtml(link.snippet || link.path || '')}</small>
                        </li>`).join('');
                    return `<section class="viewer-backlinks"><h3>Backlink</h3><ul>${rows}</ul></section>`;
                }

                // Viewer-specific: extracts mermaid blocks into placeholders,
                // returns { html, diagrams } so diagrams survive Vue re-renders
                function renderViewerMarkdown(text, context = {}) {
                    if (!text) return { html: '', diagrams: new Map() };
                    const diagrams = new Map();
                    let counter = 0;
                    const obsidianText = preprocessObsidianMarkdown(text, context);
                    const processed = obsidianText.replace(/```mermaid\n([\s\S]*?)\n```/g, (match, diagram) => {
                        const id = `viewer-mermaid-${counter++}`;
                        diagrams.set(id, diagram.trim());
                        return `<div id="${id}" class="viewer-mermaid-placeholder">📊 Caricamento diagramma...</div>`;
                    });
                    return { html: marked.parse(processed), diagrams };
                }

                async function postRenderViewer(diagrams) {
                    await nextTick();
                    await nextTick(); // secondo tick: assicura rendering Vue completato
                    const el = document.querySelector('.viewer-content');
                    if (!el) return;
                    viewerDiagramSources.clear();
                    for (const [id, diagram] of diagrams) {
                        const ph = document.getElementById(id);
                        if (!ph || !diagram) continue;
                        try {
                            const renderId = `render-${id}-${Date.now()}`;
                            const { svg } = await mermaid.render(renderId, diagram);
                            viewerDiagramSources.set(id, diagram);
                            ph.innerHTML = `<div class="viewer-mermaid-container">
                                ${svg}
                                <button class="viewer-mermaid-popup-btn"
                                    onclick="openViewerDiagramPopup('${id}')"
                                    title="Apri a schermo intero">🔍 Apri</button>
                            </div>`;
                            ph.className = 'viewer-mermaid';
                        } catch (e) {
                            ph.innerHTML = `<div class="viewer-mermaid-error">❌ Errore diagramma: <code>${e.message || 'Unknown'}</code></div>`;
                        }
                    }
                    if (window.renderMathInElement) {
                        renderMathInElement(el, {
                            delimiters: [
                                { left: '$$', right: '$$', display: true },
                                { left: '$',  right: '$',  display: false },
                                { left: '\\(', right: '\\)', display: false },
                                { left: '\\[', right: '\\]', display: true }
                            ],
                            throwOnError: false
                        });
                    }
                }

                function scrollToBottom() {
                    nextTick(() => {
                        if (chatContent.value) {
                            chatContent.value.scrollTop = chatContent.value.scrollHeight;
                        }
                    });
                }

                function autoResize() {
                    const el = inputArea.value;
                    if (el) {
                        el.style.height = 'auto';
                        el.style.height = Math.min(el.scrollHeight, 120) + 'px';
                    }
                }

                async function sendMessage() {
                    const query = userInput.value.trim();
                    if (!query || isStreaming.value) return;

                    userInput.value = '';
                    nextTick(() => autoResize());

                    messages.value.push({ role: 'user', content: query });
                    scrollToBottom();

                    messages.value.push({ role: 'assistant', content: '', streaming: true });
                    const msgIdx = messages.value.length - 1;
                    openLatestChatTurn();
                    isStreaming.value = true;
                    scrollToBottom();

                    try {
                        const response = await fetch('/chat/query', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({
                                model: 'default',
                                prompt: query,
                                stream: true,
                                retrieval: !chatWithoutRetrieval.value,
                                options: { temperature: 0.7, max_tokens: 2000 },
                                sessionId: sessionId.value
                            })
                        });

                        if (!response.ok) {
                            messages.value[msgIdx].content = `Server error: ${response.status}`;
                            return;
                        }

                        const reader = response.body.getReader();
                        const decoder = new TextDecoder();
                        let buffer = '';

                        while (true) {
                            const { done, value } = await reader.read();
                            if (done) break;

                            buffer += decoder.decode(value, { stream: true });
                            const lines = buffer.split('\n');
                            buffer = lines.pop();

                            for (const line of lines) {
                                if (line.startsWith('data: ')) {
                                    try {
                                        const data = JSON.parse(line.slice(6));
                                        if (data.response) {
                                            messages.value[msgIdx].content += data.response;
                                            scrollToBottom();
                                        }
                                    } catch (e) {}
                                }
                            }
                        }

                        if (buffer.startsWith('data: ')) {
                            try {
                                const data = JSON.parse(buffer.slice(6));
                                if (data.response) messages.value[msgIdx].content += data.response;
                            } catch (e) {}
                        }

                    } catch (e) {
                        messages.value[msgIdx].content = 'Connection error. Is the server running?';
                    } finally {
                        messages.value[msgIdx].streaming = false;
                        isStreaming.value = false;
                        scrollToBottom();
                        loadContextUsage();   // history grew → refresh the occupancy badge
                    }
                }

                function clearChat() {
                    messages.value = [];
                    openChatTurn.value = null;
                    userInput.value = '';
                }

                function copyLast() {
                    const last = [...messages.value].reverse().find(m => m.role === 'assistant');
                    if (last) navigator.clipboard.writeText(last.content);
                }

                function _noteTimestamp() {
                    const d = new Date();
                    return d.toISOString().slice(0,10) + '-' + d.toTimeString().slice(0,8).replace(/:/g,'');
                }

                async function saveMessageToNote(msg) {
                    const name = 'chat-msg-' + _noteTimestamp();
                    try {
                        const res = await fetch('/notes', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ name, content: msg.content })
                        });
                        if (!res.ok) throw new Error('HTTP ' + res.status);
                        await loadNotes();
                        alert(`Saved as note: ${name}`);
                    } catch (e) {
                        alert('Note save error: ' + e.message);
                    }
                }

                async function saveChatToNote() {
                    if (messages.value.length === 0) return;
                    const name = 'chat-' + _noteTimestamp();
                    const lines = messages.value.map(m =>
                        m.role === 'user'
                            ? `**Utente:** ${m.content}`
                            : `**KubiRAG:**\n\n${m.content}`
                    );
                    const content = lines.join('\n\n---\n\n');
                    try {
                        const res = await fetch('/notes', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ name, content })
                        });
                        if (!res.ok) throw new Error('HTTP ' + res.status);
                        await loadNotes();
                        alert(`Chat saved as note: ${name}`);
                    } catch (e) {
                        alert('Note save error: ' + e.message);
                    }
                }

                async function switchChatSession(id) {
                    sessionId.value = id;
                    messages.value = [];
                    openChatTurn.value = null;
                    try {
                        const res = await fetch(`/chat/sessions/${encodeURIComponent(id)}`);
                        if (res.ok) {
                            const data = await res.json();
                            messages.value = (data.messages || []).map(m => ({
                                role: m.role,
                                content: m.content,
                                streaming: false
                            }));
                            openLatestChatTurn();
                        }
                    } catch (e) { /* session may be empty */ }
                    if (typeof setHeaderButtonValue === 'function') setHeaderButtonValue('btnChatsVal', id);
                    await nextTick();
                    if (chatContent.value) chatContent.value.scrollTop = chatContent.value.scrollHeight;
                }

                onMounted(() => {
                    refreshSources();
                    loadNotes();
                    loadArtifacts();
                    loadNoteWatchStatus();
                    loadFolders();
                    loadBounds();
                    loadTermIndex();
                    loadPinned();
                    loadContextUsage();
                    loadEvidenceStatus();
                    loadKnowledgeScopeContext();
                    window.refreshSources = refreshSources;
                    window.loadNotes = loadNotes;
                    window.loadArtifacts = loadArtifacts;
                    window.resetNoteTagFilter = resetNoteTagFilter;
                    window.loadFolders = loadFolders;
                    window.loadBounds = loadBounds;
                    window.openSettingsPanel = openSettingsPanel;
                    window.switchChatSession = switchChatSession;
                    // Opened from the plain-JS Notebooks list ("Edit" per row) for a specific store.
                    window.openNotebookProperties = openNotebookModal;
                    const requestedConcept = new URLSearchParams(window.location.search).get('concept');
                    if (requestedConcept) {
                        openConceptsPanel().then(() => openConceptDetail(requestedConcept));
                    }
                    // Wiki links live inside v-html, where Vue's @click never binds — the shared
                    // module delegates from `document` instead, surviving every re-render.
                    if (window.KubiWikiLinks) window.KubiWikiLinks.onNavigate(target => { openWikiLink(target); });
                    // Returning from a full-page sub-app (Think/Flashcards/markboard) via the
                    // browser Back button can restore this page from the bfcache without re-running
                    // onMounted, leaving the Notes panel stale. Refresh it on bfcache restore.
                    window.addEventListener('pageshow', (e) => {
                        if (e.persisted) { loadNotes(); loadArtifacts(); loadFolders(); loadPinned(); loadContextUsage(); }
                    });
                });

                watch(() => noteWatch.value.running, running => {
                    if (running) startNoteWatchPolling(); else stopNoteWatchPolling();
                });

                onUnmounted(stopNoteWatchPolling);

                const appState = {
                    // chat
                    messages, chatTurns, openChatTurn, openChatTurnItem,
                    userInput, isStreaming, sessionId, chatWithoutRetrieval,
                    chatContent, inputArea,
                    renderMarkdown, parseRagResponse, sendMessage, clearChat, copyLast, autoResize,
                    saveMessageToNote, saveChatToNote, suggestedQuestions, askSuggestedQuestion,
                    // sources + upload
                    leftTab, sources, sourcesLoading, sourceFilter, filteredSources,
                    termIndex, termIndexLoaded, termMatchCount, luceneSearchActive,
                    refreshSources, loadTermIndex, docExt, insertDocRef,
                    uploading, uploadError, uploadSuccess, draggingOver, fileInputRef,
                    pdfModal, confirmPdfChunked,
                    addSourceModal, scrapeDocument,
                    uploadDocument, handleFileSelect, handleDrop, handleDragOver, handleDragLeave,
                    sourceTree, openFolders, draggingOverFolder,
                    isFolderOpen, toggleFolder, closeAllFolders,
                    handleFolderDrop, handleFolderDragOver, handleFolderDragLeave,
                    // settings
                    settingsOpen, settingsLoading, settingsSaved, settings, openSettingsPanel, saveSettings,
                    contextUsage, loadContextUsage,
                    availableModels, modelsLoading, loadModels,
                    // viewer
                    viewerOpen, viewerLoading, viewerDoc, viewerHtml, viewerEnrichment, viewDoc,
                    enrichmentRunning, enrichmentProgress, startEnrichment, stopEnrichment,
                    enrichScopeOpen, enrichScope, openEnrichModal, confirmStartEnrichment,
                    evidencePanelOpen, evidenceLoading, evidenceDiscovering,
                    evidenceItems, evidenceListTotal, evidenceFilter, filteredEvidenceItems, evidenceStatus, evidenceMessage, evidenceScope, conceptDetail, conceptDetailLoading,
                    conceptIdentity, conceptPermalinkForm, conceptPermalinkBusy, previewConceptPermalink, promoteConceptPermalink, copyIdentity,
                    termIdentities, termIdentity, termIdentityFilter, termIdentitiesLoading, filteredTermIdentities,
                    termPermalinkForm, termPermalinkBusy, selectTermIdentity, previewTermPermalink, promoteTermPermalink,
                    knowledgeScopeMode, activeNotebookScope, changeKnowledgeScope,
                    conceptLinkForm, conceptInsertForm, conceptAliasForm, conceptRenameForm,
                    openEvidencePanel, discoverEvidence, addWebEvidence, rejectEvidence, acceptEvidence, writeConceptNote, editConceptNote,
                    openConceptDetail, linkConceptSource, unlinkConceptSource, insertConceptWikiLink,
                    addConceptAlias, removeConceptAlias, renameSelectedConcept, removeSelectedConcept,
                    acceptConceptSuggestion, rejectConceptSuggestion, openEvidenceSource,
                    conceptRelationForm, conceptPredicateOptions, conceptTargetOptions, conceptRelationBusy,
                    searchConceptPredicates, searchConceptTargets, selectConceptPredicate, selectConceptTarget,
                    conceptPredicateLabel, addConceptRelation, removeConceptRelation,
                    // concepts panel (bilink [[ ]] view)
                    conceptsPanelOpen, conceptsLoading, conceptsList, conceptsTotal, conceptsFilter, conceptsBilinkOnly, conceptRemoving, conceptRenaming,
                    conceptTags, selectedConceptTag, selectConceptTag,
                    filteredConcepts, openConceptsPanel, conceptsView, selectConceptsView,
                    conceptMapDetail, conceptMapLoading, conceptMapError, conceptMapData, conceptMapContainer,
                    conceptMapSearch, conceptMapFocusId, loadConceptMap, selectConceptMapDetail,
                    selectConceptMapNode, focusConceptMap, fitConceptMap, okfBusy, okfExport, okfImport,
                    // pinned
                    pinnedDocs, pinnedPanelOpen, loadPinned, isPinned, togglePin, unpinDocument, deleteSource, deletePdfGroup,
                    isFolderAllPinned, isFolderAnyPinned, togglePinFolder,
                    // mindmap
                    mindmapOpen, mindmapLoading, mindmapData, mindmapError, mindmapContainer, openMindmap,
                    mindmapTab, mindmapManualExists, mindmapEditing, mindmapSource, mindmapSaving, mindmapManualContainer,
                    selectMindmapTab, editManualMindmap, previewManualMindmap, saveManualMindmap, deleteManualMindmap,
                    // sunburst
                    sunburstOpen, sunburstLoading, sunburstData, sunburstError, sunburstContainer,
                    sunburstBreadcrumb, sunburstScope, sunburstMode, sunburstTopics, sunburstDepth,
                    sunburstGranularity, sunburstSelected,
                    openSunburst, sunburstNavigateTo, refreshSunburst, focusSunburstNode, openSunburstDocument,
                    wikiGenerating, wikiModalOpen, wikiContentSections, wikiOutputDir, wikiOverwrite,
                    wikiPageIds, wikiPageIdsBusy,
                    openWikiModal, generateWiki, refreshWikiPageIds, prepareWikiPageIds, useWikiPublicationPreset,
                    notebookModalOpen, notebookSaving, notebookStoreName, notebookDefaultTitle, notebookProps, notebookPreview, openNotebookModal, saveNotebookProperties,
                    bookGenerating, bookModalOpen, bookTitle, bookEyebrow, bookSubtitle, bookAuthor, bookCoverImage, bookIncludeCover, bookIncludeToc, bookScope, bookOutputDir, openBookModal, generateBook, printBook,
                    slideModalOpen, slideGenerating, slideOutputDir, openSlideModal, generateSlides,
                    openFlashcards,
                    openQuiz,
                    openThink,
                    openReport,
                    openCanvas,
                    // notes
                    notes, notesLoading, artifacts, artifactsLoading, artifactLegacyCount, artifactMigrationBusy, artifactMigrationMessage, artifactMigrationItems,
                    artifactIcon, openArtifact, duplicateArtifact, renameArtifact, deleteArtifact, migrateArtifacts,
                    noteForm, notePublication, noteWatch, noteWatchBusy, notePublishAllBusy,
                    noteTags, selectedNoteTag, selectNoteTag,
                    openNoteForm, closeNoteForm, saveNote, deleteNote, viewNote,
                    openNotePublication, closeNotePublication, publishSelectedNote, publishAllNotes,
                    noteRelations, noteLinkSuggestions, outgoingNoteLinks, outgoingLinkTitle, openOutgoingLink,
                    openNoteRelations, closeNoteRelations, previewNotePermalink, promoteNotePermalink,
                    addManagedWikiLink, removeManagedWikiLink, addNoteSourceLink,
                    removeNoteSourceLink, saveNoteRelations, openWikiLink, openNoteFromPanel,
                    noteWatchReportOpen, openNoteWatchReport, noteWatchChangeLabel, formatNoteWatchTime,
                    loadNoteWatchStatus, startNoteWatch, stopNoteWatch, scanNoteWatch, refreshChangedNotes,
                    isNotePinned, togglePinNote,
                    openInMarkview,
                    // folder browser
                    centerTab, folders, foldersLoading, currentFolderPath, folderParent,
                    newFolderName, folderCrumbs, currentFolderName,
                    loadFolders, goToFolder, selectFolder, createFolder, openFoldersTab,
                    // bounds browser
                    bounds, boundsLoading, selectedBound, boundResources, boundResourcesLoading,
                    boundPath, boundParent, boundUpdates, boundImporting, boundMessage, boundCrumbs,
                    loadBounds, selectBound, loadBoundResources, loadBoundUpdates, importBoundResource,
                    addSelectedBoundAsSource, openBoundsTab, prettyJson
                };
                provide('appState', appState);
                return appState;
            }
        }).mount('#app');

        // Legacy panels remain plain DOM because they use inline global callbacks.
        // Insert their fragment only after Vue has mounted the reactive interface.
        const legacyResponse = await fetch('/web/components/legacy-modals.html');
        if (!legacyResponse.ok) {
            throw new Error(`Unable to load legacy panels: HTTP ${legacyResponse.status}`);
        }
        document.body.insertAdjacentHTML('beforeend', await legacyResponse.text());

        const panelsScript = document.createElement('script');
        panelsScript.src = '/web/panels.js';
        document.body.appendChild(panelsScript);
