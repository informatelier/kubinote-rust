#!/usr/bin/env python3
"""Ollama finto e deterministico per registrare i golden HTTP (stesso comportamento di
`kubirag_acceptance::fake_ollama` in Rust).

Uso: fake_ollama.py <porta>

- GET  /api/tags        → due modelli, `qwen3:4b` e `nomic-embed-text`
- POST /api/show        → `qwen3.context_length` 40960 per qwen3, nessun context_length per gli altri
- POST /api/generate    → tre frammenti NDJSON: "Risposta ", "di prova", "." (rerank JSON: {"ranking":[1]})
- POST /api/embeddings  → [1.0, lunghezza UTF-16 del testo modulo 10, 0.5]
"""
import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def utf16_len(text: str) -> int:
    return len(text.encode("utf-16-le")) // 2


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def _send(self, status: int, body: bytes, content_type: str = "application/json"):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _json(self):
        length = int(self.headers.get("Content-Length") or 0)
        return json.loads(self.rfile.read(length) or b"{}")

    def do_GET(self):
        if self.path.endswith("/api/tags"):
            body = {"models": [{"name": "qwen3:4b", "model": "qwen3:4b", "size": 1},
                               {"name": "nomic-embed-text", "model": "nomic-embed-text", "size": 1}]}
            return self._send(200, json.dumps(body).encode())
        self._send(404, b"{}")

    def do_POST(self):
        request = self._json()
        if self.path.endswith("/api/show"):
            name = request.get("model") or request.get("name") or ""
            info = {"general.architecture": "qwen3", "qwen3.context_length": 40960} if name.startswith("qwen3") \
                else {"general.architecture": "nomic-bert"}
            body = {"modelfile": "", "template": "", "license": "", "parameters": "",
                    "model_info": info, "details": {}, "capabilities": ["completion"],
                    "modified_at": "2026-01-01T00:00:00Z"}
            return self._send(200, json.dumps(body).encode())
        if self.path.endswith("/api/embeddings"):
            text = request.get("prompt", "")
            return self._send(200, json.dumps({"embedding": [1.0, float(utf16_len(text) % 10), 0.5]}).encode())
        if self.path.endswith("/api/generate"):
            if request.get("format") == "json":
                chunks = ['{"ranking":[1]}']
            else:
                chunks = ["Risposta ", "di prova", "."]
            lines = [json.dumps({"response": c, "done": False}) for c in chunks]
            lines.append(json.dumps({"response": "", "done": True}))
            return self._send(200, ("\n".join(lines) + "\n").encode(), "application/x-ndjson")
        self._send(404, b"{}")


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
