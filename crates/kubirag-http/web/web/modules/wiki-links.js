/**
 * Obsidian-style wiki links: turns [[Target]] into navigable anchors.
 *
 * Classic script (not an ES module) on purpose: it has to be consumable both by
 * app.js (loaded with type="module") and by the inline scripts of markview.html /
 * markboard.html, which are plain scripts. A global is the only shape all three share.
 *
 * Target parsing mirrors NoteRelationService.cleanWikiTarget so front-end and back-end
 * agree on what "[[a|b#c]]" points at: strip the alias after `|`, strip the anchor after `#`.
 */
(function () {
    'use strict';

    // Matches [[Target]] and captures a leading `!` so image embeds ![[img.png]] are skipped.
    // No lookbehind: Safari < 16.4 throws at parse time on (?<!!) and would kill the whole script.
    var WIKI_LINK_RE = /(!?)\[\[([^\]\n]+?)]]/g;

    // Fenced blocks and inline code spans — links must never be created inside them.
    var PROTECTED_RE = /(```[\s\S]*?```|`[^`\n]*`)/g;

    function escapeHtml(value) {
        return String(value == null ? '' : value)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    /** "Target#section|alias" -> parsed target, anchor and visible label. */
    function parseTarget(raw) {
        var parts = String(raw).split('|');
        var alias = parts.slice(1).join('|').trim();
        var destination = parts[0].trim();
        var target = destination.split('#')[0].trim();
        var anchor = destination.indexOf('#') >= 0 ? destination.substring(destination.indexOf('#') + 1).trim() : '';
        return { target: target, anchor: anchor, label: alias || destination };
    }

    function cleanRef(value) {
        return String(value || '').replace(/\\/g, '/').replace(/^\/+/, '').replace(/\.md$/i, '').replace(/^\/+|\/+$/g, '');
    }

    function noteRef(note) {
        return cleanRef(note && (note.ref || note.path || note.name));
    }

    /** Resolves an Obsidian target against the recursive /notes index. */
    function resolveNote(target, notes, sourceRef) {
        var clean = cleanRef(target);
        var list = notes || [];
        if (!clean) return { status: 'missing', note: null };
        if (clean.indexOf('/') >= 0) {
            var direct = list.filter(function (note) { return noteRef(note).toLowerCase() === clean.toLowerCase(); });
            if (direct.length === 1) return { status: 'found', note: direct[0] };
            var folder = cleanRef(sourceRef).split('/').slice(0, -1).join('/');
            var relative = cleanRef((folder ? folder + '/' : '') + clean);
            var localPath = list.filter(function (note) { return noteRef(note).toLowerCase() === relative.toLowerCase(); });
            if (localPath.length === 1) return { status: 'found', note: localPath[0] };
            return { status: direct.length + localPath.length > 1 ? 'ambiguous' : 'missing', note: null };
        }
        var matches = list.filter(function (note) { return String(note.name || '').toLowerCase() === clean.toLowerCase(); });
        if (matches.length === 1) return { status: 'found', note: matches[0] };
        if (matches.length > 1) {
            var sourceFolder = cleanRef(sourceRef).split('/').slice(0, -1).join('/').toLowerCase();
            var local = matches.filter(function (note) {
                return noteRef(note).split('/').slice(0, -1).join('/').toLowerCase() === sourceFolder;
            });
            if (local.length === 1) return { status: 'found', note: local[0] };
            return { status: 'ambiguous', note: null };
        }
        return { status: 'missing', note: null };
    }

    /**
     * Applies `fn` only to the parts of `text` outside code fences / inline code.
     * String.split with a capturing group interleaves the delimiters at odd indices.
     */
    function outsideCode(text, fn) {
        return String(text).split(PROTECTED_RE).map(function (segment, i) {
            return i % 2 === 0 ? fn(segment) : segment;
        }).join('');
    }

    /**
     * Builds one navigable note anchor. Kept public so every producer of note links —
     * inline [[ ]], the backlink lists, the relations panel — emits identical markup and is
     * picked up by the same delegated click handler.
     *
     * @param {string} target   note name the link points at
     * @param {string} label    visible text
     * @param {object} options  { href, resolved (default true), title }
     */
    function anchor(target, label, options) {
        var opts = options || {};
        var resolved = opts.resolved !== false;
        var ambiguous = opts.ambiguous === true;
        var cls = 'kubi-wikilink' + (ambiguous ? ' kubi-wikilink--ambiguous' : (resolved ? '' : ' kubi-wikilink--new'));
        var title = opts.title || (resolved ? target : target + ' — nota da creare');
        return '<a href="' + escapeHtml(opts.href || '#') + '"' +
            ' class="' + cls + '"' +
            ' data-note="' + escapeHtml(target) + '"' +
            ' data-resolved="' + (resolved ? '1' : '0') + '"' +
            ' data-ambiguous="' + (ambiguous ? '1' : '0') + '"' +
            ' title="' + escapeHtml(title) + '"' +
            '>' + escapeHtml(label) + '</a>';
    }

    /**
     * Rewrites wiki links into anchors.
     *
     * Must run *after* the frontmatter has been wrapped in a ```yaml fence by
     * preprocessObsidianMarkdown — that way frontmatter values are protected for free.
     *
     * @param {string} text     markdown source
     * @param {object} options
     *   known {Set<string>}    lowercase note names that exist; omit to treat every link as resolved
     *   href  {function}       (target, resolved) => string — the anchor's real href
     */
    function render(text, options) {
        var opts = options || {};
        var known = opts.known || null;
        var notes = opts.notes || null;
        var href = opts.href || function () { return '#'; };
        return outsideCode(String(text || ''), function (segment) {
            return segment.replace(WIKI_LINK_RE, function (match, bang, raw) {
                if (bang) return match; // ![[...]] is an embed, handled elsewhere
                var parsed = parseTarget(raw);
                if (!parsed.target) return match;
                var resolution = notes ? resolveNote(parsed.target, notes, opts.sourceRef) : null;
                var resolved = resolution ? resolution.status === 'found' : (known ? known.has(parsed.target.toLowerCase()) : true);
                var ambiguous = resolution && resolution.status === 'ambiguous';
                var resolvedTarget = resolution && resolution.note ? noteRef(resolution.note) : parsed.target;
                return anchor(ambiguous ? parsed.target : resolvedTarget, parsed.label, {
                    href: ambiguous ? '#' : href(resolvedTarget, resolved),
                    resolved: resolved,
                    ambiguous: ambiguous,
                    title: ambiguous ? parsed.target + ' — riferimento ambiguo' : undefined
                });
            });
        });
    }

    /**
     * The full Obsidian-flavoured markdown preprocessing, in one place — it used to be
     * copy-pasted in app.js and markview.html and missing entirely from markboard.html.
     *
     * Order matters: the frontmatter becomes a ```yaml fence first, so its values are then
     * protected from linkification for free; image embeds are consumed before plain links.
     *
     * @param {string} text
     * @param {object} context  { note, doc } — resolves ![[asset]] against the right endpoint
     * @param {object} options  { known, href } — forwarded to render(); omit to skip wiki links
     */
    function preprocess(text, context, options) {
        var ctx = context || {};
        var processed = String(text || '');

        processed = processed.replace(/^---\r?\n([\s\S]*?)\r?\n---(?=\r?\n|$)/, function (match, body) {
            return '```yaml\n---\n' + body.replace(/\s+$/, '') + '\n---\n```\n';
        });

        processed = processed.replace(/!\[\[([^\]]+)]]/g, function (match, rawTarget) {
            var target = rawTarget.split('|')[0].trim();
            if (!target) return match;
            var alt = rawTarget.split('|').slice(1).join('|').trim() || target.split('/').pop();
            var src = ctx.note
                ? '/notes/' + encodeURIComponent(ctx.note) + '/asset?file=' + encodeURIComponent(target) +
                    (ctx.notePath ? '&path=' + encodeURIComponent(ctx.notePath) : '')
                : ctx.doc
                    ? '/documents/' + encodeURIComponent(ctx.doc) + '/asset?file=' + encodeURIComponent(target)
                    : target;
            return '![' + alt + '](' + src + ')';
        });

        return options ? render(processed, options) : processed;
    }

    var navigateHandler = null;
    var listenerInstalled = false;

    /**
     * Registers the page's navigation behaviour and installs a single delegated
     * listener on `document`. Delegation (rather than inline onclick) survives Vue
     * re-rendering v-html content, and works identically on the plain pages.
     *
     * The handler only sees plain left clicks: ctrl/cmd/middle clicks fall through to
     * the anchor's real href, so "open in new tab" keeps working.
     *
     * @param {function} handler (target, resolved, anchor) => boolean|undefined
     *                           return false to let the browser navigate natively
     */
    function onNavigate(handler) {
        navigateHandler = handler;
        if (listenerInstalled) return;
        listenerInstalled = true;
        document.addEventListener('click', function (event) {
            if (event.defaultPrevented) return;
            if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
            var anchor = event.target && event.target.closest && event.target.closest('a.kubi-wikilink');
            if (!anchor || !navigateHandler) return;
            var target = anchor.getAttribute('data-note') || '';
            if (!target) return;
            if (anchor.getAttribute('data-ambiguous') === '1') {
                event.preventDefault();
                window.alert('Wiki link ambiguo: usa il percorso della nota per distinguerlo.');
                return;
            }
            if (navigateHandler(target, anchor.getAttribute('data-resolved') === '1', anchor) === false) return;
            event.preventDefault();
        });
    }

    /** Fetches every note name in the store (any subfolder) as a lowercase Set. */
    function loadKnownNotes() {
        return fetch('/notes?recursive=true')
            .then(function (res) { return res.ok ? res.json() : []; })
            .then(function (notes) {
                var set = new Set();
                (notes || []).forEach(function (note) {
                    if (note && note.name) set.add(String(note.name).toLowerCase());
                });
                return set;
            })
            .catch(function () { return new Set(); });
    }

    function loadNotes() {
        return fetch('/notes?recursive=true', { cache: 'no-store' })
            .then(function (res) { return res.ok ? res.json() : []; })
            .catch(function () { return []; });
    }

    /** URL of the editor prefilled to create `target` — the "lazy note creation" entry point. */
    function newNoteHref(target) {
        var ref = cleanRef(target);
        var title = ref.split('/').pop() || ref;
        return '/markboard?note=__new__&ref=' + encodeURIComponent(ref) + '&title=' + encodeURIComponent(title);
    }

    window.KubiWikiLinks = {
        preprocess: preprocess,
        render: render,
        anchor: anchor,
        onNavigate: onNavigate,
        loadKnownNotes: loadKnownNotes,
        loadNotes: loadNotes,
        newNoteHref: newNoteHref,
        parseTarget: parseTarget,
        cleanRef: cleanRef,
        noteRef: noteRef,
        resolveNote: resolveNote
    };
})();
