#!/usr/bin/env node
// Smoke test della SPA (criterio di uscita di F3: "la SPA esistente funziona senza
// modifiche"). Avvia l'Ollama finto e `kubirag server` su un workspace temporaneo con la
// fixture HTTP, carica una fonte, apre la pagina in Chromium e fa una domanda in chat.
//
// Uso: node spa_smoke.mjs <binario kubirag | kubirag-…-jar-with-dependencies.jar> [screenshot-dir]
//
// Fallisce se la pagina ha errori JavaScript, se una richiesta al server fallisce con uno
// stato diverso da 501 (route rimandate a F4–F6) o se la risposta in chat non arriva.
// Richiede `playwright` (con Chromium) e python3. Senza accesso ai CDN, KUBIRAG_CDN_DIR
// indica una cartella con i pacchetti npm estratti (`npm pack` + `tar`), es. `vue-3.5.13/`.
import { spawn } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const CDN = {
  'esm.sh/vue@3.5.13/dist/vue.esm-bundler.js': 'vue-3.5.13/package/dist/vue.esm-browser.js',
  'cdn.jsdelivr.net/npm/d3@7/dist/d3.min.js': 'd3-7.9.0/package/dist/d3.min.js',
  'cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.js': 'katex-0.16.11/package/dist/katex.min.js',
  'cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css': 'katex-0.16.11/package/dist/katex.min.css',
  'cdn.jsdelivr.net/npm/katex@0.16.11/dist/contrib/auto-render.min.js': 'katex-0.16.11/package/dist/contrib/auto-render.min.js',
  'cdn.jsdelivr.net/npm/marked/marked.min.js': 'marked/package/lib/marked.umd.js',
  'cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.min.js': 'mermaid/package/dist/mermaid.min.js',
  'cdn.jsdelivr.net/npm/html-to-image@1.11.11/dist/html-to-image.js': 'html-to-image-1.11.11/package/dist/html-to-image.js',
};

const [server, shots] = process.argv.slice(2);
if (!server) {
  console.error('uso: node spa_smoke.mjs <binario kubirag | jar> [screenshot-dir]');
  process.exit(2);
}
const tools = dirname(fileURLToPath(import.meta.url));
const freePort = () => new Promise(done => {
  const s = createServer().listen(0, '127.0.0.1', () => { const { port } = s.address(); s.close(() => done(port)); });
});
const sleep = ms => new Promise(done => setTimeout(done, ms));
async function waitUntilUp(url) {
  for (let i = 0; i < 240; i++) {
    try { await fetch(url); return; } catch { await sleep(250); }
  }
  throw new Error(`${url} non risponde`);
}

const root = mkdtempSync(join(tmpdir(), 'kubirag-spa-'));
const ws = join(root, 'ws'), home = join(root, 'home');
cpSync(join(tools, '../golden/http/fixture'), ws, { recursive: true });
mkdirSync(home);
const [ollamaPort, port] = [await freePort(), await freePort()];
const ollama = `http://127.0.0.1:${ollamaPort}/api`;
mkdirSync(join(ws, '.qb'));
writeFileSync(join(ws, '.qb/qb-rag.json'), JSON.stringify({ ollamaBaseUrl: ollama, modelName: 'qwen3:4b' }));

const env = { ...process.env, OLLAMA_BASE_URL: ollama, HOME: home, USERPROFILE: home };
const python = process.platform === 'win32' ? 'python' : 'python3';
const children = [spawn(python, [join(tools, 'fake_ollama.py'), String(ollamaPort)], { stdio: 'inherit' })];
const command = server.endsWith('.jar')
  ? ['java', [`-Duser.home=${home}`, '-jar', resolve(server)]]
  : [resolve(server), []];
children.push(spawn(command[0], [...command[1], 'server', 'kubirag', '--port', String(port)],
  { cwd: ws, env, stdio: 'inherit' }));
const base = `http://127.0.0.1:${port}`;
let failures = [];
try {
  await waitUntilUp(`${ollama}/tags`);
  await waitUntilUp(`${base}/governance`);
  const etag = (await fetch(`${base}/governance`)).headers.get('etag');
  const form = new FormData();
  form.append('file', new Blob([readFileSync(join(ws, 'docs/ralph-loop-quickstart.md'))]), 'ralph-loop-quickstart.md');
  form.append('targetDir', 'docs');
  const upload = await fetch(`${base}/documents/upload`, { method: 'POST', body: form, headers: { 'If-Match': etag } });
  if (upload.status !== 201) failures.push(`upload della fonte: ${upload.status}`);

  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
  const cdnDir = process.env.KUBIRAG_CDN_DIR;
  if (cdnDir) {
    await page.route(url => !url.href.startsWith(base), route => {
      const url = new URL(route.request().url());
      const file = CDN[url.host + url.pathname];
      return file
        ? route.fulfill({ path: join(cdnDir, file), contentType: file.endsWith('.css') ? 'text/css' : 'application/javascript' })
        : route.fulfill({ status: 204, body: '' });
    });
  }
  const deferred = new Set();
  page.on('pageerror', e => failures.push(`errore JavaScript: ${e.message}`));
  page.on('response', r => {
    const url = new URL(r.url());
    if (url.origin !== base) return;
    const call = `${r.request().method()} ${url.pathname}`;
    if (r.status() === 501) deferred.add(call);
    else if (r.status() >= 400) failures.push(`${call} → ${r.status()}`);
  });
  await page.goto(`${base}/`, { waitUntil: 'networkidle' });
  const sources = page.getByText('Sources');
  if (!(await sources.count())) failures.push('la SPA non mostra il pannello Sources');
  if (shots) await page.screenshot({ path: join(shots, 'spa-home.png') });
  const box = page.locator('textarea').first();
  await box.fill("che cos'è ralph loop?");
  await box.press('Enter');
  try {
    await page.getByText('Risposta di prova.').waitFor({ timeout: 15000 });
    await page.getByText('1 fonte recuperata').waitFor({ timeout: 5000 });
  } catch {
    failures.push('la chat non mostra la risposta con la fonte recuperata');
  }
  if (shots) await page.screenshot({ path: join(shots, 'spa-chat.png') });
  await browser.close();
  console.log(`route rimandate chiamate dalla SPA (501): ${[...deferred].sort().join(', ') || 'nessuna'}`);
} finally {
  for (const child of children) child.kill();
}
if (failures.length) {
  console.error(`smoke test della SPA fallito:\n- ${failures.join('\n- ')}`);
  process.exit(1);
}
console.log('smoke test della SPA superato');
