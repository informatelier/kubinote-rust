'use strict';

const $ = selector => document.querySelector(selector);
const $$ = selector => [...document.querySelectorAll(selector)];
const MAX_NODES = 20;
const EMPTY_CANVAS = () => ({ kubiRag: { version: 1 }, nodes: [], edges: [] });
const state = { artifacts: [], resources: [], active: null, compare: false, sync: false, importCandidate: null, closing: false, preparingIdentities: false };

function uid(prefix) {
    return `${prefix}-${crypto.randomUUID ? crypto.randomUUID() : Date.now() + '-' + Math.random().toString(16).slice(2)}`;
}

async function request(url, options = {}) {
    const response = await fetch(url, options);
    const data = await response.json().catch(() => ({}));
    if (!response.ok) throw Object.assign(new Error(data.error || `HTTP ${response.status}`), { status: response.status, data });
    return data;
}

function toast(message) {
    const element = $('#toast');
    element.textContent = message;
    element.classList.add('show');
    clearTimeout(toast.timer);
    toast.timer = setTimeout(() => element.classList.remove('show'), 2600);
}

function chooseEdge(edge = {}) {
    const dialog = $('#edge-dialog');
    const label = $('#edge-label');
    label.value = edge.label || '';
    dialog.returnValue = '';
    dialog.showModal();
    label.focus();
    return new Promise(resolve => {
        dialog.addEventListener('close', () => {
            if (!['directed', 'undirected'].includes(dialog.returnValue)) return resolve(null);
            resolve({ label: label.value.trim(), directed: dialog.returnValue === 'directed' });
        }, { once: true });
    });
}

async function loadCatalog() {
    state.artifacts = (await request('/artifacts', { cache: 'no-store' })).filter(item => item.kind === 'canvas');
    renderCatalog();
    $$('.canvas-select').forEach(select => fillCanvasSelect(select));
}

function renderCatalog() {
    const list = $('#canvas-list');
    list.replaceChildren();
    if (!state.artifacts.length) {
        const empty = document.createElement('p'); empty.className = 'hint'; empty.textContent = 'No Canvas yet.'; list.append(empty); return;
    }
    state.artifacts.forEach(artifact => {
        const card = document.createElement('article'); card.className = 'canvas-card';
        const icon = document.createElement('div'); icon.className = 'canvas-icon'; icon.textContent = '🕸️';
        const title = document.createElement('h3'); title.textContent = artifact.name;
        const meta = document.createElement('p'); meta.textContent = new Date(artifact.modifiedAt).toLocaleString();
        const actions = document.createElement('div'); actions.className = 'canvas-card-actions';
        [['Open', () => openWorkspace(artifact.ref)], ['Rename', () => renameCanvas(artifact)], ['Duplicate', () => duplicateCanvas(artifact)], ['Delete', () => deleteCanvas(artifact)]].forEach(([label, handler]) => {
            const button = document.createElement('button'); button.textContent = label; button.addEventListener('click', handler); actions.append(button);
        });
        card.append(icon, title, meta, actions); list.append(card);
    });
}

async function createCanvas() {
    const name = prompt('Canvas name:');
    if (!name?.trim()) return;
    const created = await request('/artifacts', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: name.trim(), type: 'canvas', content: JSON.stringify(EMPTY_CANVAS(), null, 2) })
    });
    await loadCatalog();
    await openWorkspace(created.ref);
}

async function renameCanvas(artifact) {
    const newName = prompt('New name:', artifact.name);
    if (!newName?.trim() || newName.trim() === artifact.name) return;
    await request(`/artifacts/rename?ref=${encodeURIComponent(artifact.ref)}`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ newName: newName.trim(), expectedRevision: artifact.revision })
    });
    await loadCatalog();
}

async function duplicateCanvas(artifact) {
    const name = prompt('Copy name:', `${artifact.name}-copy`);
    if (!name?.trim()) return;
    const source = await request(`/artifacts/content?ref=${encodeURIComponent(artifact.ref)}`, { cache: 'no-store' });
    await request('/artifacts', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: name.trim(), type: 'canvas', content: source.content }) });
    await loadCatalog();
}

async function deleteCanvas(artifact) {
    if (!confirm(`Delete Canvas "${artifact.name}"? Notes and Concepts will not be removed.`)) return;
    await request(`/artifacts/content?ref=${encodeURIComponent(artifact.ref)}`, { method: 'DELETE' });
    await loadCatalog();
}

function fillCanvasSelect(select) {
    const current = select.value;
    select.replaceChildren(new Option('Select Canvas…', ''));
    state.artifacts.forEach(item => select.add(new Option(item.name, item.ref)));
    if (state.artifacts.some(item => item.ref === current)) select.value = current;
}

async function loadResources(refresh = false) {
    state.resources = await request(`/canvas-api/resources${refresh ? '?refresh=true' : ''}`, { cache: 'no-store' });
    const unavailableNotes = state.resources.filter(resource => resource.kind === 'note' && !resource.available).length;
    $('#prepare-note-ids').hidden = unavailableNotes === 0;
    $('#prepare-note-ids').textContent = `Enable ${unavailableNotes} legacy note${unavailableNotes === 1 ? '' : 's'}`;
    renderResources($('#resource-search').value);
    boards.forEach(board => board.resolveResources());
}

function renderResources(query = '') {
    const list = $('#resource-list'); list.replaceChildren();
    const needle = query.trim().toLowerCase();
    const board = activeBoard();
    const filtered = state.resources.filter(resource => !needle || resource.label.toLowerCase().includes(needle) || resource.ref.toLowerCase().includes(needle));
    $('#resource-count').textContent = needle ? `${filtered.length} of ${state.resources.length} resources` : `${filtered.length} resources`;
    filtered.forEach(resource => {
        const row = document.createElement('div'); row.className = 'resource-item';
        const icon = document.createElement('span'); icon.textContent = resource.kind === 'note' ? '📄' : '◆';
        const text = document.createElement('div');
        const title = document.createElement('div'); title.textContent = resource.label;
        const kind = document.createElement('small'); kind.textContent = resource.available ? resource.kind : `${resource.kind} · identity required`;
        text.append(title, kind);
        const present = Boolean(resource.uri) && board?.doc.nodes.some(node => node.kubi?.resourceUri === resource.uri);
        const add = document.createElement('button'); add.textContent = present ? '✓' : '+';
        add.disabled = present || state.preparingIdentities;
        add.title = present ? `${resource.label} is already on this Canvas` : `Add ${resource.label}`;
        if (!resource.available) add.title = 'Assign a stable note identity and add it';
        add.addEventListener('click', () => resource.available ? activeBoard()?.addResource(resource) : prepareLegacyNotes(resource));
        row.append(icon, text, add); list.append(row);
    });
}

class CanvasBoard {
    constructor(shell) {
        this.shell = shell;
        this.board = shell.querySelector('.board');
        this.world = shell.querySelector('.world');
        this.nodesLayer = shell.querySelector('.node-layer');
        this.edgesLayer = shell.querySelector('.edge-layer');
        this.select = shell.querySelector('.canvas-select');
        this.status = shell.querySelector('.save-state');
        this.markerId = `arrow-${shell.dataset.side}`;
        this.ref = ''; this.revision = ''; this.doc = EMPTY_CANVAS();
        this.viewport = { x: 40, y: 40, scale: 1 };
        this.selected = new Set(); this.selectedEdges = new Set(); this.connectSource = null; this.saveTimer = null;
        this.changeVersion = 0; this.saving = false; this.loadGeneration = 0;
        this.bind(); this.applyViewport();
    }

    bind() {
        this.select.addEventListener('change', () => {
            const next = this.select.value;
            if (!next) return;
            state.active = this; renderResources($('#resource-search').value);
            if (this.saveState === 'dirty' || this.saveState === 'conflict') {
                if (!confirm('Discard local changes and load another Canvas?')) { this.select.value = this.ref; return; }
            }
            this.load(next).catch(error => toast(error.message));
        });
        this.shell.querySelector('[data-action=reload]').addEventListener('click', () => {
            if (!this.ref) return;
            if ((this.saveState === 'dirty' || this.saveState === 'conflict') && !confirm('Reload and discard local changes?')) return;
            this.load(this.ref).catch(error => toast(error.message));
        });
        this.shell.querySelector('[data-action=fit]').addEventListener('click', () => this.fit());
        this.shell.querySelector('[data-action=connect]').addEventListener('click', () => this.beginConnect());
        this.shell.querySelector('[data-action=edit]').addEventListener('click', () => this.editSelected());
        this.shell.querySelector('[data-action=color]').addEventListener('input', event => this.applySelectedColor(event.target.value));
        this.shell.querySelector('[data-action=remove]').addEventListener('click', () => this.removeSelected());
        this.board.addEventListener('wheel', event => this.zoom(event), { passive: false });
        this.board.addEventListener('pointerdown', event => this.panStart(event));
        this.board.addEventListener('keydown', event => this.keydown(event));
        this.board.addEventListener('focus', () => { state.active = this; renderResources($('#resource-search').value); });
    }

    async load(ref) {
        const generation = ++this.loadGeneration;
        clearTimeout(this.saveTimer);
        const data = await request(`/artifacts/content?ref=${encodeURIComponent(ref)}`, { cache: 'no-store' });
        if (generation !== this.loadGeneration) return;
        this.ref = data.ref; this.revision = data.revision; this.doc = JSON.parse(data.content);
        this.changeVersion = 0;
        this.selected.clear(); this.selectedEdges.clear(); this.connectSource = null;
        this.select.value = data.ref; this.setStatus('saved'); this.render(); this.fit();
        renderResources($('#resource-search').value);
        $('#document-title').textContent = state.compare ? 'Canvas comparison' : data.name;
        $('#export-button').disabled = false;
    }

    render() {
        const focusedNodeId = this.nodesLayer.contains(document.activeElement) ? document.activeElement.dataset.id : '';
        this.nodesLayer.replaceChildren(); this.edgesLayer.replaceChildren();
        const defs = document.createElementNS('http://www.w3.org/2000/svg', 'defs');
        defs.innerHTML = `<marker id="${this.markerId}" markerWidth="10" markerHeight="10" refX="9" refY="4" orient="auto"><path class="arrow-head" d="M0,0 L0,8 L10,4 z"/></marker>`;
        this.edgesLayer.append(defs);
        this.doc.edges.forEach(edge => this.renderEdge(edge));
        this.doc.nodes.forEach(node => this.renderNode(node));
        this.shell.querySelector('.empty-help').hidden = this.doc.nodes.length > 0;
        this.updateColorControl();
        if (focusedNodeId) this.nodesLayer.querySelector(`[data-id="${CSS.escape(focusedNodeId)}"]`)?.focus({ preventScroll: true });
    }

    renderNode(node) {
        const resource = state.resources.find(item => item.uri === node.kubi?.resourceUri);
        const element = document.createElement('div');
        element.className = 'node'; element.dataset.id = node.id; element.dataset.kind = node.kubi?.resourceKind || '';
        element.style.left = `${node.x}px`; element.style.top = `${node.y}px`; element.style.width = `${node.width}px`; element.style.height = `${node.height}px`;
        element.style.setProperty('--node-color', node.color || '#668fd1');
        element.classList.toggle('unresolved', !resource); element.classList.toggle('selected', this.selected.has(node.id));
        element.classList.toggle('connect-source', this.connectSource === node.id);
        element.tabIndex = 0; element.setAttribute('role', 'button');
        element.setAttribute('aria-label', resource ? `${node.kubi.resourceKind}: ${node.kubi.alias || resource.label}` : `Unresolved ${node.kubi?.resourceKind || 'resource'}: ${node.kubi?.alias || node.text || node.id}`);
        const title = document.createElement('div'); title.className = 'node-title'; title.textContent = node.kubi?.alias || resource?.label || node.text || 'Unresolved';
        const meta = document.createElement('div'); meta.className = 'node-meta'; meta.textContent = resource ? node.kubi.resourceKind : 'unresolved';
        element.append(title, meta);
        element.addEventListener('pointerdown', event => this.nodePointerDown(event, node));
        element.addEventListener('click', event => this.nodeClick(event, node));
        element.addEventListener('dblclick', event => { event.stopPropagation(); this.openResource(node); });
        element.addEventListener('focus', () => {
            state.active = this; this.selected = new Set([node.id]); this.selectedEdges.clear();
            this.nodesLayer.querySelectorAll('.node').forEach(candidate => candidate.classList.toggle('selected', candidate === element));
        });
        element.addEventListener('keydown', event => {
            if (event.key === 'Enter') { event.preventDefault(); event.stopPropagation(); this.openResource(node); }
        });
        this.nodesLayer.append(element);
    }

    renderEdge(edge) {
        const from = this.doc.nodes.find(node => node.id === edge.fromNode), to = this.doc.nodes.find(node => node.id === edge.toNode);
        if (!from || !to) return;
        const directed = edge.directed || edge.toEnd === 'arrow';
        const start = this.edgeBoundary(from, to), end = this.edgeBoundary(to, from, directed ? 7 : 0);
        const x1 = start.x, y1 = start.y, x2 = end.x, y2 = end.y;
        const middleX = (x1 + x2) / 2, middleY = (y1 + y2) / 2;
        const group = document.createElementNS('http://www.w3.org/2000/svg', 'g');
        group.classList.add('edge-group'); group.classList.toggle('selected', this.selectedEdges.has(edge.id));
        const line = document.createElementNS('http://www.w3.org/2000/svg', 'line');
        line.setAttribute('x1', x1); line.setAttribute('y1', y1); line.setAttribute('x2', x2); line.setAttribute('y2', y2); line.classList.add('edge');
        if (directed) line.setAttribute('marker-end', `url(#${this.markerId})`);
        const hit = line.cloneNode(); hit.removeAttribute('marker-end'); hit.classList.remove('edge'); hit.classList.add('edge-hit');
        const selectEdge = event => {
            event.stopPropagation(); this.selectedEdges = new Set([edge.id]); this.selected.clear(); this.render();
            toast('Edge selected. Use Remove or the Delete key to delete it.');
        };
        hit.addEventListener('click', selectEdge);
        group.append(line, hit);
        if (edge.label) {
            const label = document.createElementNS('http://www.w3.org/2000/svg', 'text'); label.classList.add('edge-label');
            label.setAttribute('x', middleX); label.setAttribute('y', middleY - 14); label.textContent = edge.label; group.append(label);
        }
        const handle = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
        handle.classList.add('edge-handle'); handle.setAttribute('cx', middleX); handle.setAttribute('cy', middleY); handle.setAttribute('r', 7);
        const title = document.createElementNS('http://www.w3.org/2000/svg', 'title'); title.textContent = 'Select edge, then use Remove or Delete'; handle.append(title);
        handle.addEventListener('click', selectEdge); group.append(handle);
        this.edgesLayer.append(group);
    }

    edgeBoundary(node, target, gap = 0) {
        const center = { x: node.x + node.width / 2, y: node.y + node.height / 2 };
        const targetCenter = { x: target.x + target.width / 2, y: target.y + target.height / 2 };
        const dx = targetCenter.x - center.x, dy = targetCenter.y - center.y;
        if (dx === 0 && dy === 0) return center;
        const scaleX = dx === 0 ? Infinity : (node.width / 2) / Math.abs(dx);
        const scaleY = dy === 0 ? Infinity : (node.height / 2) / Math.abs(dy);
        const scale = Math.min(scaleX, scaleY);
        const length = Math.hypot(dx, dy);
        return { x: center.x + dx * scale + dx / length * gap, y: center.y + dy * scale + dy / length * gap };
    }

    async nodeClick(event, node) {
        event.stopPropagation(); state.active = this; this.board.focus();
        if (this.connectSource && this.connectSource !== node.id) {
            const choice = await chooseEdge();
            const source = this.connectSource; this.connectSource = null;
            if (!choice) { this.render(); return; }
            this.doc.edges.push({ id: uid('edge'), fromNode: source, toNode: node.id, label: choice.label, directed: choice.directed, toEnd: choice.directed ? 'arrow' : 'none' });
            this.changed(); return;
        }
        if (event.shiftKey) this.selected.has(node.id) ? this.selected.delete(node.id) : this.selected.add(node.id);
        else { this.selected = new Set([node.id]); this.selectedEdges.clear(); }
        this.render();
    }

    nodePointerDown(event, node) {
        if (event.button !== 0) return;
        state.active = this;
        if (!this.selected.has(node.id)) this.selected = new Set([node.id]);
        const start = { x: event.clientX, y: event.clientY };
        const origins = new Map(this.doc.nodes.filter(item => this.selected.has(item.id)).map(item => [item.id, { x: item.x, y: item.y }]));
        const move = e => {
            const dx = (e.clientX - start.x) / this.viewport.scale, dy = (e.clientY - start.y) / this.viewport.scale;
            this.doc.nodes.forEach(item => { const origin = origins.get(item.id); if (origin) { item.x = Math.round(origin.x + dx); item.y = Math.round(origin.y + dy); } });
            this.render();
        };
        const up = e => { window.removeEventListener('pointermove', move); window.removeEventListener('pointerup', up); if (e.clientX !== start.x || e.clientY !== start.y) this.changed(); };
        window.addEventListener('pointermove', move); window.addEventListener('pointerup', up, { once: true });
    }

    panStart(event) {
        if (event.button !== 0 || event.target.closest('.node, .edge-group')) return;
        state.active = this; this.board.focus();
        if (!event.shiftKey) { this.selected.clear(); this.selectedEdges.clear(); this.render(); }
        const start = { x: event.clientX, y: event.clientY, vx: this.viewport.x, vy: this.viewport.y };
        const move = e => { this.viewport.x = start.vx + e.clientX - start.x; this.viewport.y = start.vy + e.clientY - start.y; this.applyViewport(true); };
        const up = () => { window.removeEventListener('pointermove', move); window.removeEventListener('pointerup', up); };
        window.addEventListener('pointermove', move); window.addEventListener('pointerup', up, { once: true });
    }

    zoom(event) {
        event.preventDefault(); state.active = this;
        const rect = this.board.getBoundingClientRect(), old = this.viewport.scale;
        const next = Math.max(.25, Math.min(2.5, old * (event.deltaY < 0 ? 1.1 : .9)));
        const px = event.clientX - rect.left, py = event.clientY - rect.top;
        this.viewport.x = px - (px - this.viewport.x) * next / old; this.viewport.y = py - (py - this.viewport.y) * next / old; this.viewport.scale = next;
        this.applyViewport(true);
    }

    applyViewport(propagate = false) {
        this.world.style.transform = `translate(${this.viewport.x}px,${this.viewport.y}px) scale(${this.viewport.scale})`;
        if (propagate && state.sync) boards.filter(board => board !== this && !board.shell.hidden).forEach(board => { board.viewport = { ...this.viewport }; board.applyViewport(false); });
    }

    fit() {
        if (!this.doc.nodes.length) { this.viewport = { x: 40, y: 40, scale: 1 }; this.applyViewport(true); return; }
        const bounds = this.doc.nodes.reduce((b, n) => ({ minX: Math.min(b.minX, n.x), minY: Math.min(b.minY, n.y), maxX: Math.max(b.maxX, n.x + n.width), maxY: Math.max(b.maxY, n.y + n.height) }), { minX: Infinity, minY: Infinity, maxX: -Infinity, maxY: -Infinity });
        const scale = Math.max(.25, Math.min(1.4, Math.min((this.board.clientWidth - 80) / (bounds.maxX - bounds.minX), (this.board.clientHeight - 80) / (bounds.maxY - bounds.minY))));
        this.viewport = { scale, x: (this.board.clientWidth - (bounds.maxX - bounds.minX) * scale) / 2 - bounds.minX * scale, y: (this.board.clientHeight - (bounds.maxY - bounds.minY) * scale) / 2 - bounds.minY * scale };
        this.applyViewport(true);
    }

    addResource(resource, point) {
        if (!this.ref) return toast('Select a Canvas first.');
        if (this.doc.nodes.some(node => node.kubi?.resourceUri === resource.uri)) return toast(`${resource.label} is already on this Canvas.`);
        if (this.doc.nodes.length >= MAX_NODES) return toast(`Canvas supports at most ${MAX_NODES} nodes.`);
        const rect = this.board.getBoundingClientRect();
        const screen = point || { x: rect.width / 2, y: rect.height / 2 };
        const x = Math.round((screen.x - this.viewport.x) / this.viewport.scale - 95), y = Math.round((screen.y - this.viewport.y) / this.viewport.scale - 43);
        this.doc.nodes.push({ id: uid('node'), type: 'text', text: resource.label, x, y, width: 190, height: 86, color: resource.kind === 'note' ? '#668fd1' : '#9b72cf', kubi: { version: 1, resourceUri: resource.uri, resourceKind: resource.kind, alias: '' } });
        this.selected = new Set([this.doc.nodes.at(-1).id]); this.changed(); renderResources($('#resource-search').value);
    }

    beginConnect() {
        if (this.selected.size !== 1) return toast('Select one source node first.');
        this.connectSource = [...this.selected][0]; this.render(); toast('Select the destination node.');
    }

    async editSelected() {
        if (this.selectedEdges.size === 1) {
            const edge = this.doc.edges.find(item => this.selectedEdges.has(item.id));
            if (!edge) return;
            const choice = await chooseEdge(edge); if (!choice) return;
            edge.label = choice.label; edge.directed = choice.directed; edge.toEnd = choice.directed ? 'arrow' : 'none'; this.changed(); return;
        }
        if (this.selected.size !== 1) return toast('Select one node or edge.');
        const node = this.doc.nodes.find(item => this.selected.has(item.id)); if (!node) return;
        const alias = prompt('Visual alias (blank uses resource label):', node.kubi.alias || ''); if (alias === null) return;
        node.kubi.alias = alias.trim(); this.changed();
    }

    applySelectedColor(color) {
        if (this.selected.size !== 1) return;
        const node = this.doc.nodes.find(item => this.selected.has(item.id)); if (!node) return;
        node.color = color; this.changed();
    }

    updateColorControl() {
        const control = this.shell.querySelector('[data-action=color]');
        const node = this.selected.size === 1 ? this.doc.nodes.find(item => this.selected.has(item.id)) : null;
        control.disabled = !node;
        if (node && /^#[0-9a-f]{6}$/i.test(node.color || '')) control.value = node.color;
    }

    removeSelected() {
        if (!this.selected.size && !this.selectedEdges.size) return;
        const ids = new Set(this.selected);
        this.doc.nodes = this.doc.nodes.filter(node => !ids.has(node.id));
        this.doc.edges = this.doc.edges.filter(edge => !ids.has(edge.fromNode) && !ids.has(edge.toNode) && !this.selectedEdges.has(edge.id));
        this.selected.clear(); this.selectedEdges.clear(); this.changed(); renderResources($('#resource-search').value);
    }

    keydown(event) {
        if (event.key === 'Delete' || event.key === 'Backspace') { event.preventDefault(); this.removeSelected(); return; }
        if (event.key === 'Escape') { this.connectSource = null; this.selected.clear(); this.selectedEdges.clear(); this.render(); return; }
        const delta = event.shiftKey ? 10 : 2;
        const directions = { ArrowLeft: [-delta, 0], ArrowRight: [delta, 0], ArrowUp: [0, -delta], ArrowDown: [0, delta] };
        if (directions[event.key] && this.selected.size) {
            event.preventDefault(); const [dx, dy] = directions[event.key];
            this.doc.nodes.filter(node => this.selected.has(node.id)).forEach(node => { node.x += dx; node.y += dy; }); this.changed();
        }
    }

    openResource(node) {
        const resource = state.resources.find(item => item.uri === node.kubi.resourceUri);
        if (!resource) return toast('This resource is not available in the current notebook.');
        const url = resource.kind === 'note' ? `/markboard?note=${encodeURIComponent(resource.ref)}` : `/?concept=${encodeURIComponent(resource.label)}`;
        window.open(url, '_blank');
    }

    resolveResources() { if (this.ref) this.render(); }
    changed() { this.changeVersion += 1; this.render(); this.setStatus('dirty'); clearTimeout(this.saveTimer); this.saveTimer = setTimeout(() => this.save(), 700); }
    setStatus(value) { this.saveState = value; this.status.textContent = value === 'dirty' ? 'changes' : value; this.status.className = `save-state ${value}`; }
    async save() {
        if (!this.ref || this.saving) return;
        const ref = this.ref, revision = this.revision, version = this.changeVersion, generation = this.loadGeneration;
        this.saving = true;
        let saveAgain = false;
        try {
            const data = await request(`/artifacts/content?ref=${encodeURIComponent(ref)}`, {
                method: 'PUT', headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ content: JSON.stringify(this.doc, null, 2), expectedRevision: revision })
            });
            if (generation !== this.loadGeneration || ref !== this.ref) return;
            this.revision = data.revision;
            if (version === this.changeVersion) this.setStatus('saved');
            else { this.setStatus('dirty'); saveAgain = true; }
            await loadCatalog();
        } catch (error) {
            if (generation !== this.loadGeneration || ref !== this.ref) return;
            this.setStatus(error.status === 409 ? 'conflict' : 'error');
            toast(error.status === 409 ? 'Save conflict: reload this Canvas before continuing.' : error.message);
        } finally {
            this.saving = false;
            if (saveAgain) this.saveTimer = setTimeout(() => this.save(), 100);
        }
    }
}

const boards = $$('.board-shell').map(shell => new CanvasBoard(shell));
const activeBoard = () => state.active || boards.find(board => !board.shell.hidden) || boards[0];

async function createResource(kind) {
    const name = prompt(`${kind === 'note' ? 'Note' : 'Concept'} name:`); if (!name?.trim()) return null;
    try {
        let resource;
        if (kind === 'note') {
            const created = await request('/notes', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: name.trim(), content: `# ${name.trim()}\n` }) });
            await loadResources(); resource = state.resources.find(item => item.kind === 'note' && (item.ref === created.ref || item.label === created.name));
        } else {
            const description = prompt('Concept description:', name.trim()); if (description === null) return null;
            resource = await request('/canvas-api/concepts', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: name.trim(), description }) });
            await loadResources(); resource = state.resources.find(item => item.uri === resource.uri) || resource;
        }
        if (!resource) throw new Error('Created resource has no stable identity');
        return resource;
    } catch (error) { toast(error.message); return null; }
}

async function prepareLegacyNotes(resourceToAdd = null) {
    if (state.preparingIdentities) return;
    const board = activeBoard();
    state.preparingIdentities = true; $('#prepare-note-ids').disabled = true;
    renderResources($('#resource-search').value);
    try {
        await request('/wiki/page-ids/prepare', { method: 'POST' });
        await loadResources(true);
        if (resourceToAdd) {
            const identified = state.resources.find(resource => resource.kind === 'note' && resource.ref === resourceToAdd.ref);
            if (!identified?.available) throw new Error(`Could not assign a stable identity to ${resourceToAdd.label}`);
            board.addResource(identified);
        } else {
            toast('Legacy notes now have stable identities.');
        }
    } catch (error) {
        toast(error.message);
    } finally {
        state.preparingIdentities = false; $('#prepare-note-ids').disabled = false;
        renderResources($('#resource-search').value);
    }
}

async function openWorkspace(ref) {
    if (boards[0].ref && boards[0].ref !== ref && (boards[0].saveState === 'dirty' || boards[0].saveState === 'conflict')) {
        if (!confirm('Discard local changes and open another Canvas?')) return false;
    }
    $('#catalog').hidden = true; $('#workspace').hidden = false; state.active = boards[0];
    await loadResources(); $$('.canvas-select').forEach(fillCanvasSelect);
    if (boards[0].ref !== ref || boards[0].saveState === 'saved') await boards[0].load(ref);
    return true;
}

function showCatalog() {
    $('#workspace').hidden = true; $('#catalog').hidden = false; state.compare = false; state.sync = false;
    boards[1].shell.hidden = true; $('.sync-toggle').hidden = true; $('#sync-view').checked = false;
    $('#compare-button').textContent = 'Compare'; $('#document-title').textContent = 'Investigative workspace';
}
function toggleCompare() {
    if ($('#workspace').hidden) {
        if (!state.artifacts.length) return toast('Create a Canvas first.');
        openWorkspace(state.artifacts[0].ref).then(opened => { if (opened) toggleCompare(); }).catch(error => toast(error.message)); return;
    }
    state.compare = !state.compare; boards[1].shell.hidden = !state.compare; $('.sync-toggle').hidden = !state.compare;
    $('#compare-button').textContent = state.compare ? 'Single view' : 'Compare';
    if (state.compare && !boards[1].ref && state.artifacts.length) {
        boards[1].load(state.artifacts.find(item => item.ref !== boards[0].ref)?.ref || boards[0].ref).catch(error => toast(error.message));
    }
    $('#document-title').textContent = state.compare ? 'Canvas comparison' : (state.artifacts.find(item => item.ref === boards[0].ref)?.name || 'Canvas');
}

async function handleImport(file) {
    if (!file) return;
    const content = await file.text(), name = file.name.replace(/\.canvas$/i, '');
    try {
        const preview = await request('/canvas-api/import', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name, content, apply: false }) });
        state.importCandidate = { name, content };
        const i = preview.inspection, container = $('#import-preview'); container.replaceChildren();
        const lines = [`Name: ${preview.name}`, `Nodes: ${i.nodeCount}/${MAX_NODES}`, `Edges: ${i.edgeCount}`, `Resolved resources: ${i.resolvedUris.length}`, `Unresolved resources: ${i.unresolvedUris.length}`, `Name collision: ${preview.collision ? 'yes' : 'no'}`];
        lines.forEach(line => { const p = document.createElement('p'); p.textContent = line; container.append(p); });
        i.diagnostics.forEach(message => { const p = document.createElement('p'); p.className = 'status-error'; p.textContent = message; container.append(p); });
        $('#apply-import').disabled = !i.valid || preview.collision; $('#import-dialog').showModal();
    } catch (error) { toast(error.message); }
}

async function applyImport(event) {
    if (!state.importCandidate) return; event.preventDefault();
    try {
        await request('/canvas-api/import', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ ...state.importCandidate, apply: true }) });
        $('#import-dialog').close(); state.importCandidate = null; await loadCatalog(); toast('Canvas imported.');
    } catch (error) { toast(error.message); }
}

function exportCanvas() {
    const board = activeBoard(); if (!board?.ref) return;
    const blob = new Blob([JSON.stringify(board.doc, null, 2)], { type: 'application/json' });
    const link = document.createElement('a'); link.href = URL.createObjectURL(blob); link.download = board.ref.split('/').pop(); link.click(); URL.revokeObjectURL(link.href);
}

function leaveCanvas(event) {
    if (new URLSearchParams(location.search).get('window') !== '1') return;
    event.preventDefault();
    const hasUnsavedChanges = boards.some(board => ['dirty', 'conflict', 'error'].includes(board.saveState));
    if (hasUnsavedChanges && !confirm('Close Canvas and discard changes that have not been saved?')) return;
    state.closing = true;
    window.close();
    setTimeout(() => { if (!window.closed) window.location.href = '/'; }, 150);
}

$('#new-button').addEventListener('click', () => createCanvas().catch(error => toast(error.message)));
$('#canvas-back').addEventListener('click', leaveCanvas);
$('#catalog-button').addEventListener('click', showCatalog);
$('#compare-button').addEventListener('click', toggleCompare);
$('#export-button').addEventListener('click', exportCanvas);
$('#import-button').addEventListener('click', () => $('#import-file').click());
$('#import-file').addEventListener('change', event => handleImport(event.target.files[0]));
$('#apply-import').addEventListener('click', applyImport);
$('#refresh-resources').addEventListener('click', () => loadResources());
$('#resource-search').addEventListener('input', event => renderResources(event.target.value));
$('#create-note').addEventListener('click', async () => { const resource = await createResource('note'); if (resource) activeBoard().addResource(resource); });
$('#create-concept').addEventListener('click', async () => { const resource = await createResource('concept'); if (resource) activeBoard().addResource(resource); });
$('#prepare-note-ids').addEventListener('click', () => prepareLegacyNotes());
$('#sync-view').addEventListener('change', event => { state.sync = event.target.checked; if (state.sync) { boards[1].viewport = { ...boards[0].viewport }; boards[1].applyViewport(); } });
window.addEventListener('beforeunload', event => {
    if (!state.closing && boards.some(board => board.saveState === 'dirty')) { event.preventDefault(); event.returnValue = ''; }
});
window.addEventListener('storage', event => {
    if (event.key === 'kubirag-theme' && ['dark', 'purple', 'light'].includes(event.newValue)) {
        document.documentElement.dataset.theme = event.newValue;
    }
});

loadCatalog().then(() => {
    const ref = new URLSearchParams(location.search).get('ref');
    if (ref) openWorkspace(ref).catch(error => toast(error.message));
}).catch(error => { $('#catalog-message').textContent = error.message; });
