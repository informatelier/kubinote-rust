#!/usr/bin/env python3
"""Inventario statico delle route HTTP chiamate dal frontend di kubirag.

Uso: spa_routes.py <kubirag/src/main/resources> <openapi.json> <excluded-routes.json> <output.json>

Cerca fetch(...), governedFetch(...) (le scritture governate con If-Match), request(...)
(l'helper del canvas) e new EventSource(...) con URL letterali o template literal, normalizza
i segmenti dinamici in `{}` e incrocia il risultato con l'OpenAPI pubblicato. Le chiamate
costruite interamente a runtime (es. fetch(url)) non sono visibili: l'inventario è un
limite inferiore, non l'elenco completo.
"""
import json
import re
import sys
from pathlib import Path

CALL = re.compile(r"""(?<![\w.])(?:fetch|governedFetch|request|new\s+EventSource)\(\s*([`'"])(.*?)\1""", re.S)
METHOD = re.compile(r"""method\s*:\s*['"](GET|POST|PUT|DELETE|PATCH)['"]""", re.I)
# `{ method }` o `method: variabile`: il verbo si decide a runtime.
DYNAMIC_METHOD = re.compile(r"""\{\s*method\s*[,}]|method\s*:\s*[A-Za-z_$][\w$.]*""")


def arguments_end(text: str, start: int) -> int:
    """Fine della lista di argomenti della chiamata che apre a `start`: il verbo va cercato
    solo lì, non nella chiamata successiva."""
    depth, quote, i = 0, None, start
    while i < len(text):
        c = text[i]
        if quote:
            if c == "\\":
                i += 1
            elif c == quote:
                quote = None
        elif c in "'\"`":
            quote = c
        elif c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return len(text)


def normalize(raw: str) -> str | None:
    path = re.sub(r"\$\{[^}]*\}", "{}", raw).split("?")[0].split("#")[0]
    if not path.startswith("/") or path.startswith("//"):
        return None
    path = re.sub(r"/+$", "", path) or "/"
    # `${query}` attaccato in coda a un segmento è una query string, non un parametro di path.
    path = re.sub(r"(?<=[^/])\{\}$", "", path)
    return re.sub(r"\{\}[^/]*", "{}", path)


def openapi_patterns(spec: dict) -> list[tuple[str, re.Pattern]]:
    return [(p, re.compile("^" + re.sub(r"\\\{[^/]+?\\\}", r"[^/]+", re.escape(p)) + "$")) for p in spec["paths"]]


def matches(route: str, patterns) -> list[str]:
    probe = route.replace("{}", "x")
    return [path for path, rx in patterns if rx.match(probe) or path == route]


def main(resources: str, openapi: str, excluded: str, output: str) -> None:
    root = Path(resources)
    routes: dict[str, dict] = {}
    for file in sorted(root.rglob("*")):
        if file.suffix not in {".js", ".html", ".vue"} or ".min." in file.name:
            continue
        text = file.read_text(encoding="utf-8", errors="replace")
        for call in CALL.finditer(text):
            route = normalize(call.group(2))
            if route is None:
                continue
            window = text[call.end(): arguments_end(text, text.index("(", call.start()))]
            literal = METHOD.search(window)
            dynamic = DYNAMIC_METHOD.search(window)
            if literal and (not dynamic or literal.start() <= dynamic.start()):
                method = literal.group(1).upper()
            elif dynamic:
                method = "DYNAMIC"
            else:
                method = "GET"
            entry = routes.setdefault(route, {"route": route, "methods": set(), "files": set()})
            entry["methods"].add(method)
            entry["files"].add(file.relative_to(root).as_posix())

    spec = json.loads(Path(openapi).read_text(encoding="utf-8"))
    excluded_doc = json.loads(Path(excluded).read_text(encoding="utf-8"))
    # Le route escluse (asset statici, pagine) usano `$nome` per i segmenti variabili.
    excluded_patterns = [(e["path"], re.compile("^" + re.sub(r"\\\$\w+", "[^/]+", re.escape(e["path"])) + "$"))
                         for e in excluded_doc]
    patterns = openapi_patterns(spec)
    inventory = []
    for route in sorted(routes):
        entry = routes[route]
        documented = matches(route, patterns)
        inventory.append({
            "route": route,
            "methods": sorted(entry["methods"]),
            "openapi": documented,
            "excludedFromOpenapi": matches(route, excluded_patterns),
            "files": sorted(entry["files"]),
        })
    summary = {
        "source": "kubirag src/main/resources (fetch / EventSource letterali)",
        "routes": len(inventory),
        "documentedInOpenapi": sum(1 for r in inventory if r["openapi"]),
        "undocumented": [r["route"] for r in inventory if not r["openapi"] and not r["excludedFromOpenapi"]],
        "inventory": inventory,
    }
    Path(output).write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"{summary['routes']} route, {summary['documentedInOpenapi']} documentate nell'OpenAPI, "
          f"non documentate: {summary['undocumented']}")


if __name__ == "__main__":
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    main(*sys.argv[1:])
