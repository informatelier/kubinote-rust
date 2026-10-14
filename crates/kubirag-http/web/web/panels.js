        // ---- Preferences (theme) Panel ----

        function applyThemeSelection(theme) {
            document.querySelectorAll('#prefsModal .prefs-theme-card').forEach(card => {
                card.classList.toggle('selected', card.dataset.themeName === theme);
            });
        }

        function setTheme(theme) {
            document.documentElement.dataset.theme = theme;
            try { localStorage.setItem('kubirag-theme', theme); } catch (e) { /* storage unavailable */ }
            applyThemeSelection(theme);
        }

        function openPrefsPanel() {
            applyThemeSelection(document.documentElement.dataset.theme || 'dark');
            document.getElementById('prefsModal').classList.add('open');
        }

        function closePrefsPanel() {
            document.getElementById('prefsModal').classList.remove('open');
        }

        function handlePrefsOverlayClick(event) {
            if (event.target === document.getElementById('prefsModal')) closePrefsPanel();
        }

        // ---- Chats Panel ----

        async function openChatsPanel() {
            document.getElementById('chatsModal').classList.add('open');
            document.getElementById('chatNewForm').style.display = 'none';
            hideChatRenameForm();
            hideChatBranchForm();
            await loadChats();
        }

        function closeChatsPanel() {
            document.getElementById('chatsModal').classList.remove('open');
        }

        function handleChatsOverlayClick(event) {
            if (event.target === document.getElementById('chatsModal')) closeChatsPanel();
        }

        function toggleChatNewForm() {
            const form = document.getElementById('chatNewForm');
            form.style.display = form.style.display === 'none' ? 'block' : 'none';
            if (form.style.display === 'block') {
                hideChatRenameForm(); hideChatBranchForm();
                document.getElementById('chatNewId').focus();
            }
        }

        function hideChatRenameForm() {
            document.getElementById('chatRenameForm').style.display = 'none';
        }

        function hideChatBranchForm() {
            document.getElementById('chatBranchForm').style.display = 'none';
        }

        async function loadChats() {
            try {
                const res = await fetch('/chat/sessions');
                if (!res.ok) throw new Error(`HTTP ${res.status}`);
                renderChats(await res.json());
            } catch (e) {
                document.getElementById('chatsList').innerHTML =
                    `<li style="color:#f28b82;font-size:13px;">Error loading chats: ${escHtml(e.message)}</li>`;
            }
        }

        function renderChats(sessions) {
            const list = document.getElementById('chatsList');
            if (!sessions.length) {
                list.innerHTML = '<li style="color:var(--text-secondary);font-size:13px;padding:8px 0;">No chat sessions yet.</li>';
                return;
            }
            const sorted = [...sessions].sort((a, b) => b.updatedAt - a.updatedAt);
            list.innerHTML = sorted.map(s => {
                const preview = s.lastMessage ? escHtml(s.lastMessage.slice(0, 80)) + (s.lastMessage.length > 80 ? '…' : '') : '';
                const date = s.updatedAt ? new Date(s.updatedAt).toLocaleDateString() : '';
                return `<li class="nb-store-item">
                    <div class="nb-store-info">
                        <div class="nb-store-name">${escHtml(s.title || s.id)}</div>
                        <div class="nb-store-meta">${s.messageCount} msg${s.messageCount !== 1 ? 's' : ''}${date ? ' · ' + date : ''}</div>
                        ${preview ? `<div class="nb-store-meta" style="font-style:italic;opacity:0.7;">${preview}</div>` : ''}
                    </div>
                    <div class="nb-store-actions">
                        <button class="nb-action-btn btn-load" onclick="switchChat('${escHtml(s.id)}')">Load</button>
                        <button class="nb-action-btn" onclick="showChatRenameForm('${escHtml(s.id)}', '${escHtml(s.title || s.id)}')">Rename</button>
                        <button class="nb-action-btn" onclick="showChatBranchForm('${escHtml(s.id)}')">Branch</button>
                        <button class="nb-action-btn btn-del-store" onclick="deleteChat('${escHtml(s.id)}')">Del</button>
                    </div>
                </li>`;
            }).join('');
        }

        async function createChat() {
            const id = document.getElementById('chatNewId').value.trim();
            if (!id) { alert('Session name is required.'); return; }
            try {
                const res = await fetch('/chat/sessions', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ sessionId: id })
                });
                if (!res.ok) { const err = await res.json(); alert(err.error || 'Create failed'); return; }
                document.getElementById('chatNewId').value = '';
                document.getElementById('chatNewForm').style.display = 'none';
                await loadChats();
            } catch (e) {
                alert('Error creating chat: ' + e.message);
            }
        }

        async function switchChat(id) {
            if (window.switchChatSession) await window.switchChatSession(id);
            setHeaderButtonValue('btnChatsVal', id);
            closeChatsPanel();
        }

        function showChatRenameForm(id, currentTitle) {
            hideChatBranchForm();
            document.getElementById('chatNewForm').style.display = 'none';
            document.getElementById('chatRenameId').value = id;
            document.getElementById('chatRenameTitle').value = currentTitle;
            document.getElementById('chatRenameForm').style.display = 'block';
            document.getElementById('chatRenameTitle').focus();
        }

        async function confirmRenameChat() {
            const id = document.getElementById('chatRenameId').value;
            const title = document.getElementById('chatRenameTitle').value.trim();
            if (!title) { alert('Title is required.'); return; }
            try {
                const res = await fetch(`/chat/sessions/${encodeURIComponent(id)}`, {
                    method: 'PUT',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ title })
                });
                if (!res.ok) { const err = await res.json(); alert(err.error || 'Rename failed'); return; }
                hideChatRenameForm();
                await loadChats();
            } catch (e) {
                alert('Error renaming chat: ' + e.message);
            }
        }

        function showChatBranchForm(id) {
            hideChatRenameForm();
            document.getElementById('chatNewForm').style.display = 'none';
            document.getElementById('chatBranchSourceId').value = id;
            document.getElementById('chatBranchNewId').value = id + '-branch';
            document.getElementById('chatBranchForm').style.display = 'block';
            document.getElementById('chatBranchNewId').focus();
            document.getElementById('chatBranchNewId').select();
        }

        async function confirmBranchChat() {
            const sourceId = document.getElementById('chatBranchSourceId').value;
            const newId = document.getElementById('chatBranchNewId').value.trim();
            if (!newId) { alert('Branch name is required.'); return; }
            try {
                const res = await fetch(`/chat/sessions/${encodeURIComponent(sourceId)}/branch`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ newSessionId: newId })
                });
                if (!res.ok) { const err = await res.json(); alert(err.error || 'Branch failed'); return; }
                hideChatBranchForm();
                await loadChats();
                if (window.switchChatSession) await window.switchChatSession(newId);
                setHeaderButtonValue('btnChatsVal', newId);
                closeChatsPanel();
            } catch (e) {
                alert('Error branching chat: ' + e.message);
            }
        }

        async function deleteChat(id) {
            if (!confirm(`Delete chat session "${id}"?`)) return;
            try {
                const res = await fetch(`/chat/sessions/${encodeURIComponent(id)}`, { method: 'DELETE' });
                if (!res.ok) { const err = await res.json(); alert(err.error || 'Delete failed'); return; }
                await loadChats();
            } catch (e) {
                alert('Error deleting chat: ' + e.message);
            }
        }

        // ---- End Chats Panel ----

        // ---- Notebooks / Stores Panel ----

        async function openNotebooksPanel() {
            document.getElementById('notebooksModal').classList.add('open');
            document.getElementById('nbNewForm').style.display = 'none';
            await loadStores();
        }

        function closeNotebooksPanel() {
            document.getElementById('notebooksModal').classList.remove('open');
        }

        function handleNbOverlayClick(event) {
            if (event.target === document.getElementById('notebooksModal')) closeNotebooksPanel();
        }

        function toggleNbNewForm() {
            const form = document.getElementById('nbNewForm');
            form.style.display = form.style.display === 'none' ? 'block' : 'none';
            if (form.style.display === 'block') document.getElementById('nbNewName').focus();
        }

        function toggleNbImportForm() {
            const form = document.getElementById('nbImportForm');
            form.style.display = form.style.display === 'none' ? 'block' : 'none';
            if (form.style.display === 'block') document.getElementById('nbImportPath').focus();
        }

        async function loadStores() {
            try {
                const res = await fetch('/stores');
                if (!res.ok) throw new Error(`HTTP ${res.status}`);
                renderStores(await res.json());
            } catch (e) {
                document.getElementById('storesList').innerHTML =
                    `<li style="color:#f28b82;font-size:13px;">Error loading notebooks: ${escHtml(e.message)}</li>`;
            }
        }

        function renderStores(stores) {
            const list = document.getElementById('storesList');
            if (!stores.length) {
                list.innerHTML = '<li style="color:var(--text-secondary);font-size:13px;padding:8px 0;">No notebooks saved yet.</li>';
                return;
            }
            const active = stores.find(s => s.active);
            setHeaderButtonValue('btnNotebooksVal', active ? active.storeName : '');
            list.innerHTML = stores.map(s => {
                const meta = s.hasEmbeddings ? 'documents + embeddings' : 'documents only';
                return `<li class="nb-store-item ${s.active ? 'active-store' : ''}">
                    <div class="nb-store-info">
                        <div class="nb-store-name">
                            ${escHtml(s.storeName)}
                            ${s.active ? '<span class="nb-active-badge">loaded</span>' : ''}
                        </div>
                        <div class="nb-store-meta">${meta}</div>
                    </div>
                    <div class="nb-store-actions">
                        ${!s.active ? `<button class="nb-action-btn btn-load" onclick="switchStore('${escHtml(s.storeName)}')">Load</button>` : ''}
                        <button class="nb-action-btn btn-save-store" onclick="saveStore('${escHtml(s.storeName)}')">Save</button>
                        <button class="nb-action-btn btn-edit-store" onclick="editNotebookProps('${escHtml(s.storeName)}')" title="Author, cover, wiki background…">Edit</button>
                        <button class="nb-action-btn btn-del-store" onclick="deleteStore('${escHtml(s.storeName)}')">Del</button>
                    </div>
                </li>`;
            }).join('');
        }

        async function createStore() {
            const name = document.getElementById('nbNewName').value.trim();
            if (!name) { alert('Notebook name is required.'); return; }
            try {
                const res = await fetch('/stores', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ storeName: name })
                });
                if (!res.ok) {
                    const err = await res.json().catch(() => ({}));
                    const detail = err.error || err.message || `HTTP ${res.status}`;
                    if (err.status) {
                        alert(`Notebook "${name}" was created, but initialization did not complete: ${detail}`);
                        await loadStores();
                    } else {
                        alert(`Could not create notebook "${name}": ${detail}`);
                    }
                    return;
                }
                document.getElementById('nbNewName').value = '';
                document.getElementById('nbNewForm').style.display = 'none';
                setHeaderButtonValue('btnNotebooksVal', name);
                await loadStores();
                if (window.refreshSources) await window.refreshSources();
                if (window.resetNoteTagFilter) window.resetNoteTagFilter();
                if (window.loadNotes) await window.loadNotes();
                if (window.loadArtifacts) await window.loadArtifacts();
            } catch (e) {
                alert('Error creating notebook: ' + e.message);
            }
        }

        // Opens the Vue properties modal for a specific notebook (no need to load it first).
        function editNotebookProps(storeName) {
            closeNotebooksPanel();
            if (window.openNotebookProperties) window.openNotebookProperties(storeName);
            else alert('Notebook editor not ready yet.');
        }

        async function switchStore(storeName) {
            try {
                const embeddingChoice = await chooseEmbeddingCompatibility(storeName);
                if (embeddingChoice === null) return;
                const res = await fetch('/stores/active', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ storeName })
                });
                if (!res.ok) {
                    const err = await res.json().catch(() => ({}));
                    const lines = [
                        `Impossibile aprire il notebook "${storeName}"`,
                        '',
                        err.error || err.message || `Load failed (HTTP ${res.status})`
                    ];
                    const issues = Array.isArray(err.issues) ? err.issues : [];
                    if (issues.length > 0) {
                        lines.push('', 'Problemi rilevati:');
                        issues.forEach(issue => {
                            const code = issue.code ? `[${issue.code}] ` : '';
                            const detail = issue.message || issue.diagnostic || issue.path || JSON.stringify(issue);
                            lines.push(`- ${code}${detail}`);
                        });
                    }
                    if (err.correlationId) lines.push('', `Riferimento: ${err.correlationId}`);
                    if (err.eventId) lines.push(`Evento: ${err.eventId}`);
                    alert(lines.join('\n'));
                    return;
                }
                setHeaderButtonValue('btnNotebooksVal', storeName);
                if (embeddingChoice === 'rebuild-current' || embeddingChoice === 'use-profile') {
                    await rebuildEmbeddingProjection(embeddingChoice === 'use-profile');
                }
                await loadStores();
                if (window.refreshSources) await window.refreshSources();
                if (window.resetNoteTagFilter) window.resetNoteTagFilter();
                if (window.loadNotes) await window.loadNotes();
                if (window.loadArtifacts) await window.loadArtifacts();
                if (window.loadFolders) await window.loadFolders();
            } catch (e) {
                alert('Error loading notebook: ' + e.message);
            }
        }

        async function saveStore(storeName) {
            try {
                const res = await fetch(`/stores/${encodeURIComponent(storeName)}/save`, { method: 'POST' });
                if (!res.ok) { const err = await res.json(); alert(err.error || 'Save failed'); return; }
            } catch (e) {
                alert('Error saving notebook: ' + e.message);
            }
        }

        async function deleteStore(storeName) {
            if (!confirm(`Delete notebook "${storeName}"?`)) return;
            try {
                const encoded = encodeURIComponent(storeName);
                const res = await fetch(`/stores/${encoded}?confirm=${encoded}`, { method: 'DELETE' });
                if (!res.ok) { const err = await res.json(); alert(err.error || 'Delete failed'); return; }
                await loadStores();
            } catch (e) {
                alert('Error deleting notebook: ' + e.message);
            }
        }

        async function importFolder() {
            const path = document.getElementById('nbImportPath').value.trim();
            const name = document.getElementById('nbImportName').value.trim();
            if (!path) { alert('Folder path is required.'); return; }
            try {
                const res = await fetch('/stores/import-folder', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ path, storeName: name || null })
                });
                if (!res.ok) { const err = await res.json(); alert(err.error || 'Import failed'); return; }
                const data = await res.json();
                document.getElementById('nbImportPath').value = '';
                document.getElementById('nbImportName').value = '';
                document.getElementById('nbImportForm').style.display = 'none';
                setHeaderButtonValue('btnNotebooksVal', data.storeName);
                await loadStores();
                if (window.refreshSources) await window.refreshSources();
                if (window.resetNoteTagFilter) window.resetNoteTagFilter();
                if (window.loadNotes) await window.loadNotes();
                if (window.loadArtifacts) await window.loadArtifacts();
            } catch (e) {
                alert('Error importing folder: ' + e.message);
            }
        }

        let embeddingCompatibilityResolver = null;

        async function chooseEmbeddingCompatibility(storeName) {
            const statusResponse = await fetch(`/embedding-projection/${encodeURIComponent(storeName)}`);
            if (!statusResponse.ok) return 'lexical';
            const status = await statusResponse.json();
            if (status.compatible) return 'compatible';

            const previousModel = status.descriptor?.model || 'sconosciuto';
            const reasons = Array.isArray(status.reasons) ? status.reasons.join('\n- ') : '';
            document.getElementById('embeddingCompatibilitySummary').textContent =
                `Profilo notebook: ${previousModel}. Embedder configurato: ${status.configuredModel}.`;
            document.getElementById('embeddingCompatibilityReasons').textContent = reasons ? `- ${reasons}` : '';
            const profileOption = document.querySelector('#embeddingCompatibilityChoice option[value="use-profile"]');
            profileOption.disabled = !status.descriptor;
            document.getElementById('embeddingCompatibilityChoice').value = 'lexical';
            document.getElementById('embeddingCompatibilityModal').classList.add('open');
            return new Promise(resolve => { embeddingCompatibilityResolver = resolve; });
        }

        function closeEmbeddingCompatibility(choice) {
            document.getElementById('embeddingCompatibilityModal').classList.remove('open');
            const resolve = embeddingCompatibilityResolver;
            embeddingCompatibilityResolver = null;
            if (resolve) resolve(choice);
        }

        function cancelEmbeddingCompatibility() {
            closeEmbeddingCompatibility(null);
        }

        function confirmEmbeddingCompatibility() {
            closeEmbeddingCompatibility(document.getElementById('embeddingCompatibilityChoice').value);
        }

        async function rebuildEmbeddingProjection(useNotebookProfile) {
            const rebuildResponse = await fetch('/embedding-projection/rebuild', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ force: true, useNotebookProfile })
            });
            const result = await rebuildResponse.json().catch(() => ({}));
            if (!rebuildResponse.ok) {
                alert(result.error || `Rigenerazione embedding fallita (HTTP ${rebuildResponse.status})`);
                return;
            }
            const projection = result.projection || {};
            alert(
                `Embedding rigenerati con ${result.model}: ${result.completed}/${result.total}.` +
                (projection.state === 'PARTIAL' ? '\nAlcuni documenti non sono stati elaborati.' : '')
            );
        }

        // ---- End Notebooks / Stores Panel ----

        // ---- Notebook Share Panel ----

        let shareBusy = false;

        async function openSharePanel() {
            document.getElementById('shareModal').classList.add('open');
            document.getElementById('shareMessage').textContent = '';
            await loadShareStores();
        }

        function closeSharePanel() {
            if (!shareBusy) document.getElementById('shareModal').classList.remove('open');
        }

        function handleShareOverlayClick(event) {
            if (event.target === document.getElementById('shareModal')) closeSharePanel();
        }

        async function loadShareStores() {
            const list = document.getElementById('shareStoresList');
            list.innerHTML = '<div class="share-placeholder">Loading notebooks…</div>';
            try {
                const res = await fetch('/stores');
                if (!res.ok) throw new Error(`HTTP ${res.status}`);
                const stores = await res.json();
                if (!stores.length) {
                    list.innerHTML = '<div class="share-placeholder">No notebooks available.</div>';
                    return;
                }
                list.innerHTML = stores.map(store => `<label class="share-store-option">
                    <input type="checkbox" name="shareStore" value="${escHtml(store.storeName)}">
                    <span>${escHtml(store.storeName)}</span>
                    ${store.active ? '<small>active</small>' : ''}
                </label>`).join('');
            } catch (error) {
                list.innerHTML = `<div class="share-message error">${escHtml(error.message)}</div>`;
            }
        }

        function toggleAllShareStores() {
            const boxes = [...document.querySelectorAll('input[name="shareStore"]')];
            const select = boxes.some(box => !box.checked);
            boxes.forEach(box => { box.checked = select; });
        }

        function setShareBusy(busy) {
            shareBusy = busy;
            document.getElementById('shareExportBtn').disabled = busy;
            document.getElementById('shareImportBtn').disabled = busy;
            document.getElementById('shareImportFile').disabled = busy;
        }

        async function responseError(response, fallback) {
            try {
                const body = await response.json();
                return body.error || fallback;
            } catch (_) {
                return fallback;
            }
        }

        async function exportSharedNotebooks() {
            const names = [...document.querySelectorAll('input[name="shareStore"]:checked')].map(box => box.value);
            const message = document.getElementById('shareMessage');
            if (!names.length) {
                message.className = 'share-message error';
                message.textContent = 'Select at least one notebook.';
                return;
            }
            setShareBusy(true);
            message.className = 'share-message';
            message.textContent = 'Building portable archive…';
            try {
                const res = await fetch('/stores/archive/export', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ storeNames: names })
                });
                if (!res.ok) throw new Error(await responseError(res, 'Export failed'));
                const blob = await res.blob();
                const disposition = res.headers.get('Content-Disposition') || '';
                const match = disposition.match(/filename="?([^";]+)"?/i);
                const filename = match ? match[1] : 'kubirag-share.zip';
                const url = URL.createObjectURL(blob);
                const link = document.createElement('a');
                link.href = url;
                link.download = filename;
                document.body.appendChild(link);
                link.click();
                link.remove();
                URL.revokeObjectURL(url);
                const warnings = Number(res.headers.get('X-KubiRAG-Warnings')) || 0;
                message.className = warnings ? 'share-message warning' : 'share-message success';
                message.textContent = `Exported ${names.length} notebook${names.length === 1 ? '' : 's'}${warnings ? ` with ${warnings} warning(s)` : ''}.`;
            } catch (error) {
                message.className = 'share-message error';
                message.textContent = error.message;
            } finally {
                setShareBusy(false);
            }
        }

        async function importSharedNotebooks() {
            const input = document.getElementById('shareImportFile');
            const message = document.getElementById('shareMessage');
            const file = input.files && input.files[0];
            if (!file) {
                message.className = 'share-message error';
                message.textContent = 'Choose a ZIP archive first.';
                return;
            }
            setShareBusy(true);
            message.className = 'share-message';
            message.textContent = 'Validating and importing archive…';
            try {
                const form = new FormData();
                form.append('file', file);
                const mode = document.getElementById('shareImportMode')?.value || 'restore';
                form.append('mode', mode);
                form.append('actor', 'ragboard-user');
                const forkName = document.getElementById('shareForkName')?.value?.trim();
                if (mode === 'fork' && forkName) form.append('forkName', forkName);
                const res = await fetch('/stores/archive/import', { method: 'POST', body: form });
                if (!res.ok) throw new Error(await responseError(res, 'Import failed'));
                const result = await res.json();
                const warnings = result.warnings || [];
                message.className = warnings.length ? 'share-message warning' : 'share-message success';
                message.textContent = `Imported: ${result.imported.join(', ')}${warnings.length ? `. ${warnings.length} warning(s).` : '.'}`;
                input.value = '';
                await loadShareStores();
            } catch (error) {
                message.className = 'share-message error';
                message.textContent = error.message;
            } finally {
                setShareBusy(false);
            }
        }

        // ---- End Notebook Share Panel ----


        // ---- Team / Roles Panel ----

        function escHtml(str) {
            return String(str).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
        }

        function setHeaderButtonValue(spanId, value) {
            const span = document.getElementById(spanId);
            if (!span) return;
            const v = value ? String(value) : '';
            span.textContent = v.length > 14 ? v.slice(0, 13) + '…' : v;
        }

        async function openTeamPanel() {
            document.getElementById('teamModal').classList.add('open');
            hideRoleForm();
            await loadRoles();
        }

        function closeTeamPanel() {
            document.getElementById('teamModal').classList.remove('open');
        }

        function handleModalOverlayClick(event) {
            if (event.target === document.getElementById('teamModal')) closeTeamPanel();
        }

        async function loadRoles() {
            try {
                const res = await fetch('/roles');
                if (!res.ok) throw new Error(`HTTP ${res.status}`);
                renderRoles(await res.json());
            } catch (e) {
                document.getElementById('rolesList').innerHTML =
                    `<li style="color:#f28b82;font-size:13px;">Error loading roles: ${escHtml(e.message)}</li>`;
            }
        }

        function renderRoles(roles) {
            const list = document.getElementById('rolesList');
            if (!roles.length) {
                list.innerHTML = '<li style="color:var(--text-secondary);font-size:13px;padding:8px 0;">No roles defined yet.</li>';
                return;
            }
            const active = roles.find(r => r.active);
            setHeaderButtonValue('btnAssistantVal', active ? active.roleName : '');
            list.innerHTML = roles.map(r => {
                const meta = [
                    r.filePatterns && r.filePatterns.length ? `Files: ${r.filePatterns.join(', ')}` : '',
                    r.voiceName ? `Voice: ${r.voiceName}` : ''
                ].filter(Boolean).join(' · ');
                return `<li class="role-item ${r.active ? 'active-role' : ''}">
                    <div class="role-item-info">
                        <div class="role-item-name">
                            ${escHtml(r.roleName)}
                            ${r.active ? '<span class="active-badge">active</span>' : ''}
                        </div>
                        <div class="role-item-prompt">${escHtml(r.rolePrompt)}</div>
                        ${meta ? `<div class="role-item-meta">${escHtml(meta)}</div>` : ''}
                    </div>
                    <div class="role-item-actions">
                        ${!r.active ? `<button class="role-action-btn btn-use" onclick="useRole('${escHtml(r.roleName)}')">Use</button>` : ''}
                        <button class="role-action-btn" onclick="editRole(${escHtml(JSON.stringify(r))})">Edit</button>
                        <button class="role-action-btn btn-delete" onclick="deleteRole('${escHtml(r.roleName)}')">Del</button>
                    </div>
                </li>`;
            }).join('');
        }

        const ROLE_PRESETS = Object.freeze({
            'research-analyst': Object.freeze({
                roleName: 'research-analyst',
                rolePrompt: 'Act as a research analyst. Answer from the available sources, distinguish evidence from inference, cite the relevant notes or documents, and state clearly when the evidence is insufficient.'
            }),
            'socratic-tutor': Object.freeze({
                roleName: 'socratic-tutor',
                rolePrompt: 'Act as a Socratic tutor. Help me understand the material through short questions, adapt to my answers, correct misconceptions explicitly, and finish with a concise recap and one practical exercise.'
            }),
            'critical-reviewer': Object.freeze({
                roleName: 'critical-reviewer',
                rolePrompt: 'Act as a critical reviewer. Test claims against the available sources, identify contradictions, missing evidence and hidden assumptions, then propose the smallest concrete improvements in priority order.'
            })
        });

        function useRolePreset() {
            const presetName = document.getElementById('rolePreset').value;
            const preset = ROLE_PRESETS[presetName];
            if (!preset) return;
            showRoleForm();
            document.getElementById('roleName').value = preset.roleName;
            document.getElementById('rolePrompt').value = preset.rolePrompt;
            document.getElementById('roleFiles').value = '';
            document.getElementById('roleVoice').value = '';
        }

        function showRoleForm(role) {
            const form = document.getElementById('roleForm');
            form.style.display = 'block';
            document.getElementById('roleFormTitle').textContent = role ? 'Edit Role' : 'New Role';
            document.getElementById('editingRoleName').value = role ? role.roleName : '';
            document.getElementById('roleName').value = role ? role.roleName : '';
            document.getElementById('roleName').disabled = !!role;
            document.getElementById('rolePrompt').value = role ? role.rolePrompt : '';
            document.getElementById('roleFiles').value = role && role.filePatterns ? role.filePatterns.join(', ') : '';
            document.getElementById('roleVoice').value = role ? (role.voiceName || '') : '';
        }

        function hideRoleForm() {
            document.getElementById('roleForm').style.display = 'none';
        }

        function editRole(roleJson) {
            const role = typeof roleJson === 'string' ? JSON.parse(roleJson) : roleJson;
            showRoleForm(role);
        }

        async function saveRole() {
            const editing = document.getElementById('editingRoleName').value;
            const name = document.getElementById('roleName').value.trim();
            const prompt = document.getElementById('rolePrompt').value.trim();
            const filesRaw = document.getElementById('roleFiles').value.trim();
            const voice = document.getElementById('roleVoice').value.trim();

            if (!name) { alert('Role name is required.'); return; }
            if (!prompt) { alert('Prompt is required.'); return; }

            const filePatterns = filesRaw ? filesRaw.split(',').map(s => s.trim()).filter(Boolean) : [];
            const body = { roleName: name, rolePrompt: prompt, filePatterns, voiceName: voice || null };

            const url = editing ? `/roles/${encodeURIComponent(editing)}` : '/roles';
            const method = editing ? 'PUT' : 'POST';
            try {
                const res = await fetch(url, {
                    method,
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(body)
                });
                if (!res.ok) { const err = await res.json(); alert(err.error || 'Save failed'); return; }
                hideRoleForm();
                await loadRoles();
            } catch (e) {
                alert('Error saving role: ' + e.message);
            }
        }

        async function deleteRole(roleName) {
            if (!confirm(`Delete role "${roleName}"?`)) return;
            try {
                const res = await fetch(`/roles/${encodeURIComponent(roleName)}`, { method: 'DELETE' });
                if (!res.ok) { const err = await res.json(); alert(err.error || 'Delete failed'); return; }
                await loadRoles();
            } catch (e) {
                alert('Error deleting role: ' + e.message);
            }
        }

        async function useRole(roleName) {
            try {
                const res = await fetch('/roles/active', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ roleName })
                });
                if (!res.ok) { const err = await res.json(); alert(err.error || 'Failed to set role'); return; }
                setHeaderButtonValue('btnAssistantVal', roleName);
                await loadRoles();
            } catch (e) {
                alert('Error setting role: ' + e.message);
            }
        }

        // Initialize header button labels from server state on page load
        (async function initHeaderState() {
            // Chat session always starts as "default" on page load
            setHeaderButtonValue('btnChatsVal', 'default');
            try {
                const [storesRes, rolesRes] = await Promise.all([
                    fetch('/stores'),
                    fetch('/roles')
                ]);
                if (storesRes.ok) {
                    const stores = await storesRes.json();
                    const active = stores.find(s => s.active);
                    if (active) setHeaderButtonValue('btnNotebooksVal', active.storeName);
                }
                if (rolesRes.ok) {
                    const roles = await rolesRes.json();
                    const active = roles.find(r => r.active);
                    if (active) setHeaderButtonValue('btnAssistantVal', active.roleName);
                }
            } catch (e) { /* silently ignore — buttons stay with base label */ }
        })();
