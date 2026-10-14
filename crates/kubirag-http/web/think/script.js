// Fallacies Database
const fallacies = [
        {
            "id": "straw-man-fallacy",
            "name": "Straw Man",
            "description": "Misrepresenting the argument to create false conclusions.",
            "type": "fallacy"
        },
        {
            "id": "false-cause-fallacy",
            "name": "False Cause",
            "description": "Assuming that correlated events are in a cause-and-effect relationship.",
            "type": "fallacy"
        },
        {
            "id": "slippery-slope-fallacy",
            "name": "Slippery Slope",
            "description": "Assuming A falsely implies Z, so we block A.",
            "type": "fallacy"
        },
        {
            "id": "ad-hominem-fallacy",
            "name": "Ad Hominem",
            "description": "Attacking the person to ridicule the ideas.",
            "type": "fallacy"
        },
        {
            "id": "appeal-to-exception",
            "name": "Miracle",
            "description": "Seeking exceptions to support a false rule.",
            "type": "fallacy"
        },
        {
            "id": "loaded-question-fallacy",
            "name": "Loaded Question",
            "description": "Asking a question while already knowing the answer to expose an uncomfortable position.",
            "type": "fallacy"
        },
        {
            "id": "gamblers-fallacy",
            "name": "Gambler's",
            "description": "Assuming a statistical frequency for independent events.",
            "type": "fallacy"
        },
        {
            "id": "appeal-to-authority",
            "name": "Authority",
            "description": "Appealing to a title of authority.",
            "type": "fallacy"
        },
        {
            "id": "black-or-white-fallacy",
            "name": "Black or White",
            "description": "Assuming the existence of only two possibilities.",
            "type": "fallacy"
        },
        {
            "id": "begging-the-question",
            "name": "Self-Evident",
            "description": "The conclusion is included in the premise.",
            "type": "fallacy"
        },
        {
            "id": "appeal-to-nature",
            "name": "Natural",
            "description": "Assuming a natural event as an inevitable rule.",
            "type": "fallacy"
        },
        {
            "id": "fallacy-of-composition",
            "name": "Composition",
            "description": "Assuming that what is true for the parts is true for the whole.",
            "type": "fallacy"
        },
        {
            "id": "anecdotal-fallacy",
            "name": "Anecdote",
            "description": "Assuming personal experience as a rule.",
            "type": "fallacy"
        },
        {
            "id": "appeal-to-emotion",
            "name": "Emotion",
            "description": "Loading a choice with emotion.",
            "type": "fallacy"
        },
        {
            "id": "fallacy-of-faulty-chain",
            "name": "Chain of Errors",
            "description": "An error in reasoning invalidates the premise.",
            "type": "fallacy"
        },
        {
            "id": "appeal-to-incredulity",
            "name": "Incredulity",
            "description": "Difficult to believe makes a conclusion false.",
            "type": "fallacy"
        },
        {
            "id": "tu-quoque-fallacy",
            "name": "Chain of Criticism",
            "description": "Responding to criticism with criticism without progress.",
            "type": "fallacy"
        },
        {
            "id": "burden-of-proof",
            "name": "Burden of Proof",
            "description": "Asserting that the proof belongs to the one who wants to refute.",
            "type": "fallacy"
        },
        {
            "id": "equivocation-fallacy",
            "name": "Ambiguity",
            "description": "Using ambiguous words or deviating their interpretation.",
            "type": "fallacy"
        },
        {
            "id": "no-true-scotsman",
            "name": "Purity",
            "description": "Attacking the purity of the premises.",
            "type": "fallacy"
        },
        {
            "id": "cherry-picking-fallacy",
            "name": "Selection",
            "description": "Selecting successful cases to support the thesis.",
            "type": "fallacy"
        },
        {
            "id": "fallacy-of-moderation",
            "name": "Compromise",
            "description": "Assuming the middle ground as the truth.",
            "type": "fallacy"
        }
    ];

const biases = [
        {
            "id": "anchoring-bias",
            "name": "Anchoring",
            "description": "The first judgment influences subsequent judgments.",
            "type": "bias"
        },
        {
            "id": "availability-heuristic",
            "name": "Availability",
            "description": "Judgment is influenced by what is most readily available.",
            "type": "bias"
        },
        {
            "id": "backfire-effect",
            "name": "Backfire Effect",
            "description": "When your beliefs are challenged, your confidence in them strengthens.",
            "type": "bias"
        },
        {
            "id": "filling-in-gaps-bias",
            "name": "Filling-in",
            "description": "Gaps are filled by personal preferences.",
            "type": "bias"
        },
        {
            "id": "belief-bias",
            "name": "Belief",
            "description": "If you agree with a conclusion, you will seek out everything that supports it.",
            "type": "bias"
        },
        {
            "id": "bystander-effect",
            "name": "Responsible",
            "description": "In an emergency, you assume someone else will intervene.",
            "type": "bias"
        },
        {
            "id": "confirmation-bias",
            "name": "Confirmation",
            "description": "You seek evidence that justifies your point of view.",
            "type": "bias"
        },
        {
            "id": "curse-of-knowledge",
            "name": "Common Sense",
            "description": "When you understand something, you assume it is obvious to everyone.",
            "type": "bias"
        },
        {
            "id": "decline-bias",
            "name": "Decline",
            "description": "The past was better than today, tomorrow will be worse than tomorrow.",
            "type": "bias"
        },
        {
            "id": "dunning-kruger-effect",
            "name": "Dunning-Kruger",
            "description": "The more you know, the less confident you feel.",
            "type": "bias"
        },
        {
            "id": "avoidance-bias",
            "name": "Avoidance",
            "description": "You let someone else take over.",
            "type": "bias"
        },
        {
            "id": "framing-effect",
            "name": "Framing",
            "description": "You let yourself be influenced by context and mode of assumption.",
            "type": "bias"
        },
        {
            "id": "fundamental-attribution-error",
            "name": "Attribution",
            "description": "You judge others by character, yourself by situation.",
            "type": "bias"
        },
        {
            "id": "ingroup-bias",
            "name": "In-Group",
            "description": "You judge those belonging to your group more favorably.",
            "type": "bias"
        },
        {
            "id": "herd-mentality",
            "name": "Herd",
            "description": "You let the group decide.",
            "type": "bias"
        },
        {
            "id": "halo-effect",
            "name": "Halo",
            "description": "Your liking of a person influences your judgment of what they do.",
            "type": "bias"
        },
        {
            "id": "conformity-bias",
            "name": "Alignment",
            "description": "You align yourself with the majority opinion.",
            "type": "bias"
        },
        {
            "id": "just-world-hypothesis",
            "name": "Better World",
            "description": "Your preferences lead you to believe an ideal world without blame exists.",
            "type": "bias"
        },
        {
            "id": "loss-aversion",
            "name": "Negation",
            "description": "You let the fear of losing block you.",
            "type": "bias"
        },
        {
            "id": "optimism-bias",
            "name": "Optimism",
            "description": "You let your thoughts overestimate a positive outcome.",
            "type": "bias"
        },
        {
            "id": "pessimism-bias",
            "name": "Pessimism",
            "description": "You let your thoughts overestimate a negative outcome.",
            "type": "bias"
        },
        {
            "id": "placebo-effect",
            "name": "Placebo",
            "description": "You believe it solves the problem even if it does nothing.",
            "type": "bias"
        },
        {
            "id": "reactance-bias",
            "name": "Reaction",
            "description": "You do the opposite of what is requested.",
            "type": "bias"
        },
        {
            "id": "self-serving-bias",
            "name": "Self-Absolution",
            "description": "Failures are due to external factors, successes are your responsibility.",
            "type": "bias"
        },
        {
            "id": "spotlight-effect",
            "name": "Spotlight",
            "description": "You overestimate what others think and say about you.",
            "type": "bias"
        },
        {
            "id": "escalation-of-commitment",
            "name": "Increasing Loss",
            "description": "You invest to avoid abandoning what has already been invested.",
            "type": "bias"
        }
    ];

// --- APP STATE ---
let state = {
    topics: [],
    chat: []
};
let myRole = null; // 'HOST' | 'CLIENT' | null
let myName = 'Anonymous';

// --- UI REFERENCES ---
const ui = {
    app: document.getElementById('app'),
    topicsList: document.getElementById('topics-list'),
    chatMessages: document.getElementById('chat-messages'),
    chatForm: document.getElementById('chat-form'),
    chatInput: document.getElementById('chat-input'),
    btnAddTopic: document.getElementById('btn-add-topic'),
    btnImport: document.getElementById('btn-import-trigger'),
    btnExport: document.getElementById('btn-export'),
    btnSaveNote: document.getElementById('btn-save-note'),
    btnSend: document.getElementById('btn-send'),


    // Connection UI
    usernameInput: document.getElementById('username-input'),
    connInitial: document.getElementById('conn-initial'),
    connActive: document.getElementById('conn-active'),
    btnHost: document.getElementById('btn-host'),
    btnJoin: document.getElementById('btn-join'),
    hostIdInput: document.getElementById('host-id-input'),
    myIdDisplay: document.getElementById('my-id-display'),
    myRoleDisplay: document.getElementById('my-role'),
    hostInfoBlock: document.getElementById('host-info-block'),
    statusTag: document.getElementById('connection-status'),
    btnCopy: document.getElementById('btn-copy-id')
};

// --- MODAL REFERENCES ---
let currentTopicId = null;
let currentOptionId = null;
let currentArgType = 'critic';
let currentSelectedFallacy = null;

// --- NETWORK MANANGER ---
class NetworkManager {
    constructor() {
        this.peer = null;
        this.connections = []; // For Host: list of client conns
        this.hostConn = null;  // For Client: conn to host
    }

    init(id = null) {
        // Create Peer
        this.peer = new Peer(id);

        this.peer.on('open', (id) => {
            console.log('My Peer ID is: ' + id);
            this.onOpen(id);
        });

        this.peer.on('connection', (conn) => {
            if (myRole === 'HOST') {
                this.handleIncomingConnection(conn);
            } else {
                conn.close();
            }
        });

        this.peer.on('error', (err) => {
            console.error(err);
            alert('PeerJS Error: ' + err.type);
        });
    }

    onOpen(id) {
        if (myRole === 'HOST') {
            ui.myIdDisplay.value = id;
            ui.hostInfoBlock.style.display = 'block';
            ui.statusTag.textContent = 'Hosting';
            ui.statusTag.style.color = 'var(--success)';
            // Load previous state if exists
            loadLocalState();
            renderAll();
        }
    }

    // -- HOST LOGIC --
    startHosting() {
        console.log('startHosting called');
        if (!validateUser()) {
            console.log('User validation failed');
            return;
        }
        myRole = 'HOST';
        this.init(); // Auto-generated ID
        ui.btnAddTopic.disabled = false;
        ui.btnImport.disabled = false;
        ui.btnExport.disabled = false;
        ui.btnSend.disabled = false;
        ui.usernameInput.disabled = true;
        updateConnectionUI();
    }

    handleIncomingConnection(conn) {
        this.connections.push(conn);
        console.log('Client connected:', conn.peer);

        conn.on('open', () => {
            // Send current full state to new client
            conn.send({ type: 'SYNC_STATE', payload: state });
        });

        conn.on('data', (data) => {
            this.handleActionFromClient(data);
        });

        conn.on('close', () => {
            this.connections = this.connections.filter(c => c !== conn);
        });
    }

    handleActionFromClient(action) {
        // Apply action to state
        processAction(action);
        // Persist (localStorage for HOST + Notes when a board name is bound)
        saveLocalState();
        persistNote();
        // Broadcast new state to ALL clients
        this.broadcastState();
        // Re-render Host UI
        renderAll();
    }

    broadcastState() {
        this.connections.forEach(conn => {
            if (conn.open) {
                conn.send({ type: 'SYNC_STATE', payload: state });
            }
        });
    }

    // -- CLIENT LOGIC --
    joinHost(hostId) {
        if (!validateUser()) return;
        myRole = 'CLIENT';
        this.init(); // Auto-generated ID for me

        // Wait for my peer to be ready before connecting
        this.peer.on('open', () => {
            ui.statusTag.textContent = 'Connecting...';
            this.hostConn = this.peer.connect(hostId);

            this.hostConn.on('open', () => {
                console.log('Connected to Host');
                ui.statusTag.textContent = 'Connected';
                ui.statusTag.style.color = 'var(--success)';
                ui.btnAddTopic.disabled = false;
                ui.btnImport.disabled = false;
                ui.btnExport.disabled = false;
                ui.btnSend.disabled = false;
                ui.usernameInput.disabled = true;
                updateConnectionUI();
            });

            this.hostConn.on('data', (data) => {
                if (data.type === 'SYNC_STATE') {
                    state = data.payload;
                    renderAll();
                }
            });

            this.hostConn.on('close', () => {
                alert('Disconnected from Host');
                ui.statusTag.textContent = 'Offline';
            });

            this.hostConn.on('error', (err) => console.error('Conn error:', err));
        });
    }

    sendAction(action) {
        if (myRole === 'HOST' || myRole === 'SOLO') {
            // Apply locally immediately (SOLO has no peers; broadcast is a no-op)
            this.handleActionFromClient(action);
        } else if (myRole === 'CLIENT' && this.hostConn && this.hostConn.open) {
            // Send to host
            this.hostConn.send(action);
        }
    }
}

const net = new NetworkManager();

// --- LOGIC CENTRAL ---

function processAction(action) {
    if (action.type === 'ADD_TOPIC') {
        state.topics.push(action.payload);
    }
    else if (action.type === 'ADD_OPTION') {
        const topic = state.topics.find(t => t.id === action.topicId);
        if (topic) {
            if (!topic.options) topic.options = [];
            topic.options.push(action.payload);
        }
    }
    else if (action.type === 'VOTE_OPTION') {
        const topic = state.topics.find(t => t.id === action.topicId);
        if (topic && topic.options) {
            const opt = topic.options.find(o => o.id === action.optionId);
            if (opt) {
                if (!opt.votes) opt.votes = {};
                // ONE vote per author
                opt.votes[action.payload.author] = action.payload.value;
            }
        }
    }
    else if (action.type === 'ADD_ARGUMENT') {
        const topic = state.topics.find(t => t.id === action.topicId);
        if (topic && topic.options) {
            const opt = topic.options.find(o => o.id === action.optionId);
            if (opt) {
                if (!opt.arguments) opt.arguments = [];
                opt.arguments.push(action.payload);
            }
        }
    }
    else if (action.type === 'ADD_CHAT') {
        state.chat.push(action.payload);
        if (state.chat.length > 50) state.chat.shift();
    }
}

function dispatch(actionType, payload, topicId = null, optionId = null) {
    const action = { type: actionType, payload, topicId, optionId };
    net.sendAction(action);
}

// --- PERSISTENCE (HOST ONLY) & CONFIG ---
function loadConfig() {
    const storedName = localStorage.getItem('kritik_username');
    if (storedName) {
        myName = storedName;
        ui.usernameInput.value = myName;
    }
}

function saveConfig() {
    myName = ui.usernameInput.value.trim() || 'Anonymous';
    localStorage.setItem('kritik_username', myName);
}

function validateUser() {
    const name = ui.usernameInput.value.trim();
    if (!name) {
        alert('Please enter a display name first.');
        return false;
    }
    saveConfig();
    return true;
}

function loadLocalState() {
    const stored = localStorage.getItem('kritik_state_p2p');
    if (stored) {
        state = JSON.parse(stored);
    }
}

function saveLocalState() {
    if (myRole === 'HOST') {
        localStorage.setItem('kritik_state_p2p', JSON.stringify(state));
    }
}

// --- NOTES PERSISTENCE (mirrors the Flashcards "note-as-document" mechanic) ---
// A Think note is a markdown file carrying a `kubi-type: think` frontmatter marker
// plus a fenced ```think JSON block holding the whole board {topics, chat}.
let currentNoteName = null; // set once the board is bound to a saved note → enables auto-save
let currentArtifactRevision = null;
const artifactContentUrl = ref => '/artifacts/content?ref=' + encodeURIComponent(ref);

function setNoteStatus(msg) {
    const el = document.getElementById('note-status');
    if (el) el.textContent = msg || '';
}

function buildNoteMarkdown() {
    const data = { version: 1, topics: state.topics, chat: state.chat };
    return '---\nkubi-type: think\n---\n\n```think\n' + JSON.stringify(data, null, 2) + '\n```\n';
}

function parseNoteMarkdown(text) {
    const m = (text || '').match(/```think\s*([\s\S]*?)```/);
    if (!m) return null;
    try { return JSON.parse(m[1].trim()); } catch (e) { return null; }
}

async function loadFromNote(name) {
    setNoteStatus('Loading…');
    try {
        const res = await fetch(artifactContentUrl(name));
        if (!res.ok) throw new Error('HTTP ' + res.status);
        const data = await res.json();
        currentArtifactRevision = data.revision || null;
        const parsed = parseNoteMarkdown(data.content || '');
        if (parsed) {
            state.topics = Array.isArray(parsed.topics) ? parsed.topics : [];
            state.chat = Array.isArray(parsed.chat) ? parsed.chat : [];
        }
        currentNoteName = name;           // bind → subsequent changes auto-PUT
        renderAll();
        setNoteStatus('Loaded ✓');
    } catch (e) {
        setNoteStatus('Load error: ' + e.message);
    }
}

// Auto-save: silently PUT the board whenever it changes, once it has a note name.
function persistNote() {
    if (!currentNoteName) return;
    fetch(artifactContentUrl(currentNoteName), {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ content: buildNoteMarkdown(), expectedRevision: currentArtifactRevision })
    }).then(async res => {
        if (!res.ok) { setNoteStatus(res.status === 409 ? 'Save conflict' : 'Save failed'); return; }
        const data = await res.json();
        currentArtifactRevision = data.revision || null;
        setNoteStatus('Saved ✓');
    })
      .catch(() => setNoteStatus('Save error'));
}

// First save (button): create the note with a chosen name, then reload in edit mode
// so auto-save (PUT) takes over — same flow Flashcards' saveDeck() uses.
async function saveToNote() {
    if (currentNoteName) { persistNote(); return; }
    const raw = prompt('Note name for this board:', 'think-' + new Date().toISOString().slice(0, 10));
    if (raw === null) return;
    const name = raw.trim().replace(/[^\w\-./ ]/g, '_');
    if (!name) { setNoteStatus('Name required'); return; }
    setNoteStatus('Saving…');
    try {
        const res = await fetch('/artifacts', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ name, type: 'think', content: buildNoteMarkdown() })
        });
        if (res.status === 409) { setNoteStatus('Name already exists'); return; }
        if (!res.ok) { setNoteStatus('Save failed: ' + res.status); return; }
        const data = await res.json();
        window.location.href = '/think?note=' + encodeURIComponent(data.ref || data.name || name);
    } catch (e) {
        setNoteStatus('Save error: ' + e.message);
    }
}

// Standalone single-user mode: usable without hosting/joining a P2P room.
// Actions apply locally (NetworkManager.sendAction treats SOLO like HOST) and,
// once a note name is bound, auto-persist to Notes.
function enterSoloMode() {
    if (myRole) return;            // never downgrade an active HOST/CLIENT session
    myRole = 'SOLO';
    if (!ui.usernameInput.value.trim()) ui.usernameInput.value = myName || 'Anonymous';
    myName = ui.usernameInput.value.trim() || 'Anonymous';
    ui.btnAddTopic.disabled = false;
    ui.btnImport.disabled = false;
    ui.btnExport.disabled = false;
    ui.btnSend.disabled = false;
    if (ui.btnSaveNote) ui.btnSaveNote.disabled = false;
    ui.statusTag.textContent = 'Solo (local)';
    ui.statusTag.style.color = 'var(--accent)';
    renderAll();
}

// --- UI UPDATE ---
function updateConnectionUI() {
    ui.connInitial.style.display = 'none';
    ui.connActive.style.display = 'block';
    ui.myRoleDisplay.textContent = myRole;
}

function renderAll() {
    renderTopics();
    renderChat();
}

function renderTopics() {
    ui.topicsList.innerHTML = '';

    if (state.topics.length === 0) {
        ui.topicsList.innerHTML = `<div class="empty-state" style="text-align:center; margin-top: 3rem; color: var(--text-secondary);">No topics yet. Start a discussion!</div>`;
        return;
    }

    const sortedTopics = [...state.topics].sort((a, b) => b.timestamp - a.timestamp);

    sortedTopics.forEach(topic => {
        const card = document.createElement('div');
        card.className = 'topic-card';

        // Render Options
        let optionsHtml = '';
        if (topic.options && topic.options.length > 0) {
            optionsHtml = `<div class="options-list">
                ${topic.options.map(opt => {
                // Calculate Score
                const votes = opt.votes || {};
                const score = Object.values(votes).reduce((a, b) => a + b, 0);

                // Render Arguments (Feedback)
                let argsHtml = '';
                if (opt.arguments && opt.arguments.length > 0) {
                    argsHtml = `<div class="arguments-list">
                            ${opt.arguments.map(arg => {
                        let fallacyTag = '';
                        if (arg.fallacy) {
                            const f = fallacies.find(f => f.id === arg.fallacy);
                            if (f) {
                                fallacyTag = `<span class="tag ${f.type}" title="${f.description}">${f.name}</span>`;
                            }
                        }
                        if (arg.fallacy) {
                            const f = biases.find(f => f.id === arg.fallacy);
                            if (f) {
                                fallacyTag = `<span class="tag ${f.type}" title="${f.description}">${f.name}</span>`;
                            }
                        }
                        return `
                                <div class="argument-item">
                                    <div style="display:flex; justify-content:space-between; align-items:flex-start;">
                                        <div>
                                            <span class="arg-type arg-${arg.type}">${arg.type}</span>
                                            <span style="font-weight:600; color:var(--accent); font-size:0.8rem; margin-right:0.5rem;">${escapeHtml(arg.author || 'Anon')}:</span>
                                            <span style="color:#eee;">${escapeHtml(arg.content)}</span>
                                        </div>
                                        <div class="topic-actions" style="margin-top:0; font-size:0.8rem;">
                                            ${fallacyTag}
                                        </div>
                                    </div>
                                </div>`;
                    }).join('')}
                        </div>`;
                }

                return `
                    <div class="option-card">
                        <div class="option-header">
                            <div class="vote-controls">
                                <button class="btn-icon" onclick="voteOption('${topic.id}', '${opt.id}', 1)">▲</button>
                                <span class="vote-count" style="color:${score > 0 ? 'var(--success)' : (score < 0 ? 'var(--danger)' : 'inherit')}">${score}</span>
                                <button class="btn-icon" onclick="voteOption('${topic.id}', '${opt.id}', -1)">▼</button>
                            </div>
                            <div style="flex:1;">
                                <div style="font-weight:500;">${escapeHtml(opt.content)}</div>
                                <div style="font-size:0.75rem; color: #64748b;">by ${escapeHtml(opt.author)}</div>
                            </div>
                            <button class="btn-icon" style="font-size:0.8rem;" onclick="window.openArgumentModal('${topic.id}', '${opt.id}')">💬</button>
                        </div>
                        ${argsHtml}
                    </div>`;
            }).join('')}
            </div>`;
        }

        card.innerHTML = `
            <div class="topic-header">
                <div>
                    <h3 class="topic-title">${escapeHtml(topic.title)}</h3>
                    <div class="topic-meta" style="margin-top:0.25rem;">
                        <span style="color:var(--accent); margin-right:0.5rem;">${escapeHtml(topic.author || 'Anon')}</span>
                        ${escapeHtml(topic.description)}
                    </div>
                </div>
                <div style="font-size:0.8rem; color: #64748b;">${new Date(topic.timestamp).toLocaleTimeString()}</div>
            </div>
            <div class="topic-actions">
                <button class="btn-outline" onclick="window.openOptionModal('${topic.id}')" title="Propose an Option/Solution">+ Add Option</button>
            </div>
            ${optionsHtml}
        `;
        ui.topicsList.appendChild(card);
    });
}

function renderChat() {
    ui.chatMessages.innerHTML = '';
    const sortedMessages = [...state.chat].sort((a, b) => a.timestamp - b.timestamp);

    sortedMessages.forEach(msg => {
        const div = document.createElement('div');
        div.className = 'message';
        div.innerHTML = `
            <span class="msg-author">${escapeHtml(msg.author || 'Anon')}</span>
            ${escapeHtml(msg.text)}
        `;
        ui.chatMessages.appendChild(div);
    });

    ui.chatMessages.scrollTop = ui.chatMessages.scrollHeight;
}

// --- EVENT LISTENERS ---

// Connection
ui.btnHost.onclick = () => net.startHosting();
ui.btnJoin.onclick = () => {
    const id = ui.hostIdInput.value.trim();
    if (id) net.joinHost(id);
};
ui.btnCopy.onclick = () => {
    ui.myIdDisplay.select();
    document.execCommand('copy');
    alert('ID Copied!');
};

// Topic Creation
ui.btnAddTopic.onclick = () => toggleModal('modal-topic', true);

document.getElementById('submit-topic').onclick = () => {
    const title = document.getElementById('new-topic-title').value.trim();
    const desc = document.getElementById('new-topic-desc').value.trim();

    if (!title) return;

    dispatch('ADD_TOPIC', {
        id: Date.now().toString(),
        title,
        description: desc,
        author: myName, // Include Author
        timestamp: Date.now(),
        options: []
    });

    document.getElementById('new-topic-title').value = '';
    document.getElementById('new-topic-desc').value = '';
    toggleModal('modal-topic', false);
};

// Option Creation
window.openOptionModal = (topicId) => {
    currentTopicId = topicId;
    document.getElementById('option-content').value = '';
    toggleModal('modal-option', true);
};

document.getElementById('submit-option').onclick = () => {
    const content = document.getElementById('option-content').value.trim();
    if (!content) return;

    dispatch('ADD_OPTION', {
        id: Date.now().toString(),
        content,
        author: myName,
        votes: {},
        arguments: []
    }, currentTopicId);

    toggleModal('modal-option', false);
};

// Vote Logic
window.voteOption = (topicId, optionId, value) => {
    if (!validateUser()) return;
    dispatch('VOTE_OPTION', {
        author: myName,
        value: value
    }, topicId, optionId);
};

// Argument Creation
window.openArgumentModal = (topicId, optionId) => {
    currentTopicId = topicId;
    currentOptionId = optionId;
    currentSelectedFallacy = null;
    document.getElementById('selected-fallacy').textContent = '';
    document.getElementById('selected-bias').textContent = '';
    document.getElementById('arg-content').value = '';
    updateArgTypeUI('critic');
    toggleModal('modal-argument', true);
};

document.getElementById('submit-argument').onclick = () => {
    const content = document.getElementById('arg-content').value.trim();
    if (!content) return;

    dispatch('ADD_ARGUMENT', {
        id: Date.now().toString(),
        type: currentArgType,
        content,
        author: myName,
        fallacy: currentSelectedFallacy,
        timestamp: Date.now()
    }, currentTopicId, currentOptionId);

    toggleModal('modal-argument', false);
};

// Type Selectors
document.querySelectorAll('.type-selector').forEach(btn => {
    btn.onclick = (e) => {
        updateArgTypeUI(e.target.getAttribute('data-type'));
    };
});

function updateArgTypeUI(type) {
    currentArgType = type;
    document.querySelectorAll('.type-selector').forEach(btn => {
        if (btn.getAttribute('data-type') === type) {
            btn.classList.add('btn-primary');
            btn.classList.remove('btn-icon');
        } else {
            btn.classList.remove('btn-primary');
            btn.classList.add('btn-icon');
        }
    });
}

// Fallacy Modal
document.getElementById('btn-attach-fallacy').onclick = () => toggleModal('modal-fallacy', true);

// Populate Fallacy List
const fallacyListEl = document.getElementById('fallacy-list');
fallacies.forEach(f => {
    const div = document.createElement('div');
    div.className = 'fallacy-option';
    div.innerHTML = `
        <div style="font-weight:600; color:${f.type === 'fallacy' ? '#fda4af' : '#fcd34d'}">${f.name}</div>
        <div style="font-size:0.85rem; color:#94a3b8;">${f.description}</div>
    `;
    div.onclick = () => {
        currentSelectedFallacy = f.id;
        document.getElementById('selected-fallacy').textContent = `Attached: ${f.name}`;
        toggleModal('modal-fallacy', false);
    };
    fallacyListEl.appendChild(div);
});

// Bias Modal
document.getElementById('btn-attach-bias').onclick = () => toggleModal('modal-bias', true);

// Populate Bias List
const biasListEl = document.getElementById('bias-list');
biases.forEach(f => {
    const div = document.createElement('div');
    div.className = 'bias-option';
    div.innerHTML = `
        <div style="font-weight:600; color:${f.type === 'bias' ? '#fcd34d' : '#6df4af' }">${f.name}</div>
        <div style="font-size:0.85rem; color:#94a3b8;">${f.description}</div>
    `;
    div.onclick = () => {
        currentSelectedFallacy = f.id;
        document.getElementById('selected-bias').textContent = `Attached: ${f.name}`;
        toggleModal('modal-bias', false);
    };
    biasListEl.appendChild(div);
});

// Chat
ui.chatForm.onsubmit = (e) => {
    e.preventDefault();
    const text = ui.chatInput.value.trim();
    if (!text) return;

    dispatch('ADD_CHAT', {
        id: Date.now().toString(),
        text,
        author: myName, // Send current username
        timestamp: Date.now()
    });

    ui.chatInput.value = '';
};

// Save current board to Notes
if (ui.btnSaveNote) ui.btnSaveNote.onclick = () => saveToNote();

// Data Management
document.getElementById('btn-export').onclick = () => {
    const dataStr = "data:text/json;charset=utf-8," + encodeURIComponent(JSON.stringify(state, null, 2));
    const downloadAnchorNode = document.createElement('a');
    downloadAnchorNode.setAttribute("href", dataStr);
    downloadAnchorNode.setAttribute("download", "kritik_state_" + new Date().toISOString().slice(0, 10) + ".json");
    document.body.appendChild(downloadAnchorNode); // required for firefox
    downloadAnchorNode.click();
    downloadAnchorNode.remove();
};

document.getElementById('btn-import-trigger').onclick = () => {
    document.getElementById('file-import').click();
};

document.getElementById('file-import').onchange = (e) => {
    const file = e.target.files[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (event) => {
        try {
            const loadedState = JSON.parse(event.target.result);
            if (loadedState.topics && loadedState.chat) {
                state = loadedState;
                renderAll();
                // If hosting, broadcast new state
                if (myRole === 'HOST') {
                    saveLocalState();
                    net.broadcastState();
                }
                alert('State imported successfully!');
            } else {
                alert('Invalid file format: Missing topics or chat.');
            }
        } catch (err) {
            console.error(err);
            alert('Error parsing JSON file.');
        }
    };
    reader.readAsText(file);
    // Reset input so same file can be selected again
    e.target.value = '';
};

// Utils
window.toggleModal = (id, show) => {
    const el = document.getElementById(id);
    if (show === undefined) el.classList.toggle('active');
    else if (show) el.classList.add('active');
    else el.classList.remove('active');
};

function escapeHtml(text) {
    if (!text) return '';
    return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

function init() {
    loadConfig();
    // When opened with a ?note= param (from the Studio "Think" card or the Notes list),
    // start a usable standalone board immediately and load the note if it already exists.
    // Without a note param we keep the original behaviour: wait for a P2P connection.
    const noteParam = new URLSearchParams(window.location.search).get('note');
    if (noteParam) {
        enterSoloMode();
        if (noteParam !== '__new__') loadFromNote(noteParam);
    }
}

// Start
init();
