export const viewerDiagramSources = new Map();

export function installDiagramPopup() {
    window.openViewerDiagramPopup = function(id) {
        const source = viewerDiagramSources.get(id);
        if (!source) return;
        const popup = window.open('', '_blank', 'width=1100,height=750,scrollbars=yes,resizable=yes');
        if (!popup) return;
        const escaped = source.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
        popup.document.write(`<!DOCTYPE html>
<html lang="it">
<head>
    <meta charset="UTF-8">
    <title>Diagramma</title>
    <script src="/mermaid-11-9-0.min.js"><\/script>
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body { background: #12121f; display: flex; flex-direction: column; height: 100vh; font-family: 'Segoe UI', sans-serif; }
        .toolbar { display: flex; gap: 8px; padding: 10px 16px; background: #1e1e2e; border-bottom: 1px solid #333; align-items: center; }
        .toolbar-title { color: #c9d1d9; font-weight: 600; font-size: 14px; flex: 1; }
        .btn { padding: 5px 14px; border: none; border-radius: 6px; background: #6366f1; color: white; cursor: pointer; font-size: 12px; font-weight: 500; }
        .btn:hover { background: #5254cc; }
        .btn-close { background: #3d3d4e; }
        .btn-close:hover { background: #555; }
        .diagram-wrap { flex: 1; overflow: auto; display: flex; justify-content: center; align-items: flex-start; padding: 24px; }
        .mermaid { background: #ffffff; border-radius: 10px; padding: 20px; display: inline-block; width:100% }
        .source-block { display: none; padding: 12px 16px 16px; }
        .source-block.show { display: block; }
        .source-block pre { background: #1e1e2e; color: #a8d4f5; font-family: monospace; font-size: 12px;
            padding: 12px; border-radius: 8px; white-space: pre-wrap; border-left: 3px solid #6366f1;
            max-height: 180px; overflow: auto; }
    </style>
</head>
<body>
    <div class="toolbar">
        <span class="toolbar-title">📊 Diagramma</span>
        <button class="btn" onclick="downloadSVG()">💾 SVG</button>
        <button class="btn" onclick="downloadPNG()">🖼️ PNG</button>
        <button class="btn" id="btnSrc" onclick="toggleSrc()">📄 Sorgente</button>
        <button class="btn btn-close" onclick="window.close()">✕ Chiudi</button>
    </div>
    <div class="diagram-wrap"><div id="diagram" class="mermaid"></div></div>
    <div class="source-block" id="srcBlock"><pre>${escaped}</pre></div>
    <script>
        mermaid.initialize({ startOnLoad: false, theme: 'neutral', securityLevel: 'loose' });
        const src = ${JSON.stringify(source)};
        mermaid.render('popup-diagram', src).then(({ svg }) => {
            document.getElementById('diagram').innerHTML = svg;
        });
        function downloadSVG() {
            const svg = document.querySelector('#diagram svg');
            if (!svg) return;
            const blob = new Blob([new XMLSerializer().serializeToString(svg)], { type: 'image/svg+xml' });
            const a = Object.assign(document.createElement('a'), { href: URL.createObjectURL(blob), download: 'diagram.svg' });
            a.click(); URL.revokeObjectURL(a.href);
        }
        function downloadPNG() {
            const svg = document.querySelector('#diagram svg');
            if (!svg) return;
            const url = URL.createObjectURL(new Blob([new XMLSerializer().serializeToString(svg)], { type: 'image/svg+xml;charset=utf-8' }));
            const img = new Image();
            img.onload = () => {
                const canvas = document.createElement('canvas');
                canvas.width = img.width; canvas.height = img.height;
                const ctx = canvas.getContext('2d');
                ctx.fillStyle = 'white'; ctx.fillRect(0, 0, canvas.width, canvas.height);
                ctx.drawImage(img, 0, 0);
                canvas.toBlob(b => {
                    const a = Object.assign(document.createElement('a'), { href: URL.createObjectURL(b), download: 'diagram.png' });
                    a.click(); URL.revokeObjectURL(a.href);
                });
                URL.revokeObjectURL(url);
            };
            img.src = url;
        }
        function toggleSrc() {
            const block = document.getElementById('srcBlock');
            const btn = document.getElementById('btnSrc');
            const show = block.classList.toggle('show');
            btn.textContent = show ? '📄 Nascondi' : '📄 Sorgente';
        }
    <\/script>
</body>
</html>`);
        popup.document.close();
    };
}
