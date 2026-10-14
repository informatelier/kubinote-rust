#!/usr/bin/env python3
"""Registra lo scenario HTTP di `golden/http/scenario.json` contro il server Kotlin di kubirag.

Uso: http_golden.py <kubirag-jar-with-dependencies.jar> <contracts/golden>

Prepara un workspace temporaneo (fixture `golden/http/fixture`, `.qb/qb-rag.json` che punta a
un Ollama finto), avvia `fake_ollama.py` e `java -jar … server <store>`, esegue i passi in
ordine e scrive `golden/http/kotlin-exchanges.json` già normalizzato. Le regole di
normalizzazione sono le stesse di `kubirag_acceptance::http` (Rust):

- i path del workspace e della home diventano `<ws>` e `<home>` (separatori `/`);
- `correlationId` e `auditEventId` diventano segnaposto fissi, gli altri UUID `<uuid-N>`
  numerati per prima apparizione nell'intero scenario, un UUID troncato in fondo a un testo
  `<uuid-partial>`;
- `timestamp`/`updatedAt` non nulli diventano `<millis>`, gli istanti ISO `<instant>`;
- gli asset statici sono confrontati con SHA-256 e lunghezza.
"""
import hashlib
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

HEADERS = ["content-type", "etag", "x-qb-revision", "x-qb-source-reused", "x-qb-source-reindexed",
           "x-session-limit"]
UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
# Un UUID troncato (es. `lastMessage` tagliato a 100 caratteri) in fondo al testo.
PARTIAL_UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}(-[0-9a-f]{0,4}){0,2}(-[0-9a-f]{0,11})?$")
INSTANT = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z")
FIXED_KEYS = {"correlationId": "<correlation>", "auditEventId": "<audit-event>"}
MILLIS_KEYS = {"timestamp", "updatedAt"}


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class Normalizer:
    def __init__(self, roots: list[tuple[str, str]]):
        self.roots = roots
        self.uuids: dict[str, str] = {}

    def text(self, value: str) -> str:
        for path, label in self.roots:
            if path in value:
                value = value.replace(path, label)
        if "<ws>" in value or "<home>" in value:
            value = value.replace("\\", "/")
        value = INSTANT.sub("<instant>", value)
        return PARTIAL_UUID.sub("<uuid-partial>", UUID.sub(self._uuid, value))

    def _uuid(self, match: re.Match) -> str:
        return self.uuids.setdefault(match.group(0), f"<uuid-{len(self.uuids) + 1}>")

    def value(self, value, key: str | None = None):
        if key in FIXED_KEYS and isinstance(value, str):
            return FIXED_KEYS[key]
        if key in MILLIS_KEYS and isinstance(value, int) and not isinstance(value, bool) and value != 0:
            return "<millis>"
        if isinstance(value, dict):
            return {k: self.value(v, k) for k, v in value.items()}
        if isinstance(value, list):
            return [self.value(v) for v in value]
        if isinstance(value, str):
            return self.text(value)
        return value


def multipart(fields: dict, ws: Path) -> tuple[bytes, str]:
    boundary = "----kubirag-golden-boundary"
    out = bytearray()
    for name, field in fields.items():
        out += f"--{boundary}\r\n".encode()
        if isinstance(field, dict):
            data = (ws / field["fixture"]).read_bytes() if "fixture" in field else field["content"].encode()
            out += f'Content-Disposition: form-data; name="{name}"; filename="{field["filename"]}"\r\n'.encode()
            out += b"Content-Type: application/octet-stream\r\n\r\n" + data + b"\r\n"
        else:
            out += f'Content-Disposition: form-data; name="{name}"\r\n\r\n{field}\r\n'.encode()
    out += f"--{boundary}--\r\n".encode()
    return bytes(out), f"multipart/form-data; boundary={boundary}"


def substitute(value, variables: dict):
    if isinstance(value, str):
        for k, v in variables.items():
            value = value.replace("{" + k + "}", v)
        return value
    if isinstance(value, dict):
        return {k: substitute(v, variables) for k, v in value.items()}
    if isinstance(value, list):
        return [substitute(v, variables) for v in value]
    return value


def pointer(document, path: str):
    for part in path.strip("/").split("/"):
        document = document[int(part)] if isinstance(document, list) else document[part]
    return document


def request(base: str, method: str, path: str, headers: dict, body: bytes | None):
    req = urllib.request.Request(base + path, data=body, method=method, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=60) as response:
            return response.status, {k.lower(): v for k, v in response.headers.items()}, response.read()
    except urllib.error.HTTPError as error:
        return error.code, {k.lower(): v for k, v in error.headers.items()}, error.read()


def decode_body(step: dict, headers: dict, raw: bytes, normalizer: Normalizer):
    if step.get("static"):
        return {"sha256": hashlib.sha256(raw).hexdigest(), "length": len(raw)}
    text = raw.decode("utf-8")
    if text.startswith("data: "):
        frames = [json.loads(frame[len("data: "):]) for frame in text.split("\n\n") if frame]
        return {"sse": normalizer.value(frames)}
    try:
        return {"json": normalizer.value(json.loads(text))}
    except json.JSONDecodeError:
        return {"text": normalizer.text(text)}


def run(jar: str, golden: Path) -> None:
    scenario = json.loads((golden / "http/scenario.json").read_text(encoding="utf-8"))
    tmp = Path(tempfile.mkdtemp(prefix="kubirag-http-")).resolve()
    ws, home = tmp / "ws", tmp / "home"
    shutil.copytree(golden / "http/fixture", ws)
    home.mkdir()
    ollama_port, server_port = free_port(), free_port()
    ollama = f"http://127.0.0.1:{ollama_port}/api"
    (ws / ".qb").mkdir()
    (ws / ".qb/qb-rag.json").write_text(json.dumps({"ollamaBaseUrl": ollama, "modelName": "qwen3:4b"}))
    tools = Path(__file__).resolve().parent
    processes = [subprocess.Popen([sys.executable, str(tools / "fake_ollama.py"), str(ollama_port)])]
    env = dict(os.environ, OLLAMA_BASE_URL=ollama, HOME=str(home), LC_ALL="C.UTF-8")
    log = open(tmp / "server.log", "w")
    processes.append(subprocess.Popen(
        ["java", f"-Duser.home={home}", "-jar", jar, "server", scenario["store"], "--port", str(server_port)],
        cwd=ws, env=env, stdout=log, stderr=subprocess.STDOUT))
    base = f"http://127.0.0.1:{server_port}"
    try:
        for _ in range(120):
            try:
                urllib.request.urlopen(base + "/governance", timeout=2)
                break
            except OSError:
                time.sleep(0.5)
        normalizer = Normalizer([(str(ws), "<ws>"), (str(home), "<home>")])
        variables = {"ollama": ollama}
        exchanges = []
        for step in scenario["steps"]:
            path = substitute(step["path"], variables)
            headers = dict(substitute(step.get("headers", {}), variables))
            body = None
            if "json" in step:
                body = json.dumps(substitute(step["json"], variables)).encode()
                headers["Content-Type"] = "application/json"
            elif "multipart" in step:
                body, headers["Content-Type"] = multipart(step["multipart"], ws)
            if_match = step.get("ifMatch")
            if if_match in ("current", "W/current"):
                _, h, _ = request(base, "GET", step.get("revisionFrom", "/governance"), {}, None)
                if_match = ("W/" if if_match.startswith("W/") else "") + h["etag"]
            if if_match is not None:
                headers["If-Match"] = if_match
            status, response_headers, raw = request(base, step["method"], path, headers, body)
            recorded = {
                "name": step["name"],
                "status": status,
                "headers": {h: response_headers[h] for h in HEADERS if h in response_headers},
                **decode_body(step, response_headers, raw, normalizer),
            }
            exchanges.append(recorded)
            for name, json_pointer in step.get("capture", {}).items():
                variables[name] = pointer(json.loads(raw), json_pointer)
        files = {}
        for name in scenario["files"]:
            file = ws / name
            if file.exists():
                files[name] = normalizer.value(json.loads(file.read_text(encoding="utf-8")))
        output = {"generator": "kubirag 09aba65, contracts/tools/http_golden.py", "exchanges": exchanges,
                  "files": files}
        target = golden / "http/kotlin-exchanges.json"
        target.write_text(json.dumps(output, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"{len(exchanges)} scambi registrati in {target}")
    finally:
        for process in processes:
            process.terminate()
        log.close()
        print(f"workspace: {tmp}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    run(sys.argv[1], Path(sys.argv[2]))
