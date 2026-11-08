#!/usr/bin/env python3
"""Local, explicit software-factory workflow for the spec implementation plan."""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import os
import urllib.request
import urllib.error
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PLAN = ROOT / "docs/specs/IMPLEMENTATION_PLAN.md"
MATRIX = ROOT / "docs/specs/SPEC_REQUIREMENTS_MATRIX.md"
REGISTRY = ROOT / "docs/specs/FACTORY_SLICES.json"
STATE = ROOT / ".software-factory/state.json"
ID = re.compile(r"(?:G\d{2}|Q\d{2}|P\d{2}[ab](?:\.\d+)?)\Z")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_digest(path: Path) -> str:
    # Git may check Markdown out with CRLF on Windows; compare normalized text.
    return hashlib.sha256(path.read_text(encoding='utf-8').encode('utf-8')).hexdigest()


def extract_registry() -> dict:
    text = PLAN.read_text(encoding="utf-8")
    rows = re.findall(r"^\| ([GPQ]\d{2}[ab]?) \| ([^|]*) \| ([^\n]*)\|$", text, re.M)
    packages = {}
    for item, deps, scope in rows:
        if item in packages:
            raise ValueError(f"Doppeltes Paket: {item}")
        packages[item] = {"id": item, "kind": "package" if item.startswith("P") else "slice", "dependencies": re.findall(r"\b[GPQ]\d{2}[ab]?\b", deps), "scope": scope.strip(), "requirements": [], "children": []}
    for item, spec, scope in re.findall(r"^\| (P(\d{2})[ab]\.\d+) \| (.*) \|$", text, re.M):
        parent = item.rsplit(".", 1)[0]
        if parent not in packages or item in packages:
            raise ValueError(f"Ungültiger Slice: {item}")
        packages[parent]["children"].append(item)
        packages[item] = {"id": item, "kind": "slice", "parent": parent, "dependencies": [], "scope": scope.strip(), "spec": int(spec), "requirements": sorted(set(re.findall(r"FR-(?:\d{3}-)?\d{3}", scope))), "children": []}
    matrix = MATRIX.read_text(encoding='utf-8')
    sections = re.split(r'(?m)^## Spec (\d+):[^\n]*\n', matrix)
    for index in range(1, len(sections), 2):
        number = int(sections[index])
        package = packages[f'Q{number:02d}']
        package['spec'] = number
        package['requirements'] = sorted(set(re.findall(r'(?m)^\| (SC-(?:\d{3}-)?\d{3}) \|', sections[index + 1])))
        if len(package['requirements']) != 8:
            raise ValueError(f'Acht Erfolgskriterien erwartet: Q{number:02d}')
    for item in list(packages.values()):
        if item["kind"] == "package":
            children = sorted(item["children"])
            if len(children) != 3:
                raise ValueError(f"Drei Teilslices erwartet: {item['id']}")
            for index, child in enumerate(children):
                packages[child]["dependencies"] = list(item["dependencies"]) + ([children[index - 1]] if index else [])
    registry = {"version": 1, "source": {"plan": str(PLAN.relative_to(ROOT)).replace('\\', '/'), "plan_sha256": source_digest(PLAN), "matrix_sha256": source_digest(MATRIX)}, "items": list(packages.values())}
    validate(registry)
    return registry


def validate(registry: dict) -> None:
    if registry.get("version") != 1 or not isinstance(registry.get("items"), list):
        raise ValueError("Unbekanntes Registerformat")
    items = registry["items"]
    by_id = {x["id"]: x for x in items}
    if len(by_id) != len(items) or any(not ID.fullmatch(x) for x in by_id):
        raise ValueError("Doppelte oder ungültige IDs")
    for item in items:
        for dep in item["dependencies"] + item["children"]:
            if dep not in by_id:
                raise ValueError(f"Unbekannte Abhängigkeit {dep} in {item['id']}")
    visiting, done = set(), set()
    def visit(item):
        if item in visiting:
            raise ValueError(f"Zyklische Abhängigkeit: {item}")
        if item in done:
            return
        visiting.add(item)
        node = by_id[item]
        # A package is complete only when its children are complete.
        for dep in (node["children"] if node["kind"] == "package" else node["dependencies"]):
            visit(dep)
        visiting.remove(item)
        done.add(item)
    for item in by_id:
        visit(item)


def load_registry() -> dict:
    registry = json.loads(REGISTRY.read_text(encoding="utf-8"))
    validate(registry)
    if registry["source"]["plan_sha256"] != source_digest(PLAN) or registry["source"]["matrix_sha256"] != source_digest(MATRIX):
        raise ValueError("Plan oder Matrix geändert: Register mit 'generate' aktualisieren")
    return registry


def load_state() -> dict:
    if not STATE.exists():
        return {"version": 1, "slices": {}}
    state = json.loads(STATE.read_text(encoding="utf-8"))
    if state.get("version") != 1 or not isinstance(state.get("slices"), dict):
        raise ValueError("Ungültiger Status")
    return state


def complete(item: str, nodes: dict, state: dict) -> bool:
    node = nodes[item]
    if node["kind"] == "package":
        return all(complete(child, nodes, state) for child in node["children"])
    record = state["slices"].get(item, {})
    if record.get('status') != 'complete' or not record.get('evidence'):
        return False
    evidence = (ROOT / record['evidence']).resolve()
    return evidence.is_relative_to(ROOT.resolve()) and evidence.is_file() and record.get('evidence_sha256') == digest(evidence)


def ready(node: dict, nodes: dict, state: dict) -> bool:
    return node["kind"] == "slice" and state['slices'].get(node['id'], {}).get('status') != 'blocked' and not complete(node["id"], nodes, state) and all(complete(d, nodes, state) for d in node["dependencies"])


def command_output(*args: str) -> str:
    result = subprocess.run(args, cwd=ROOT, text=True, encoding="utf-8", errors="replace", stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    return result.stdout.strip() if result.returncode == 0 else f"nicht verfügbar (Exit {result.returncode})"


def save_state(state: dict) -> None:
    STATE.parent.mkdir(parents=True, exist_ok=True)
    temp = STATE.with_suffix('.tmp')
    temp.write_text(json.dumps(state, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    temp.replace(STATE)


def draft(node: dict) -> str:
    prompt = ('Erstelle einen konkreten Implementierungsvorschlag für diesen Slice. '
              'Befolge AGENTS.md, erfinde keine Testergebnisse und führe keine Befehle aus.\n'
              + json.dumps(node, ensure_ascii=False) + '\n\n' + (ROOT / 'AGENTS.md').read_text(encoding='utf-8'))
    errors = []
    for provider in ('QWEN', 'DEEPSEEK'):
        key, base, model = (os.getenv(provider + suffix) for suffix in ('_API_KEY', '_BASE_URL', '_MODEL'))
        if not all((key, base, model)):
            errors.append(provider + ': nicht konfiguriert')
            continue
        url = base.rstrip('/')
        if not url.endswith('/chat/completions'):
            url += '/chat/completions'
        body = {'model': model, 'messages': [{'role': 'user', 'content': prompt}], 'max_tokens': 4000, 'temperature': 0.1}
        request = urllib.request.Request(url, data=json.dumps(body).encode('utf-8'), headers={'Content-Type': 'application/json', 'Authorization': 'Bearer ' + key})
        try:
            with urllib.request.urlopen(request, timeout=35) as response:
                result = json.load(response)
            content = result['choices'][0]['message'].get('content')
            if content:
                return f"Modell: {result.get('model', model)}\n\n{content}"
            errors.append(provider + ': leere Antwort')
        except (OSError, ValueError, KeyError, urllib.error.URLError) as exc:
            errors.append(provider + ': ' + type(exc).__name__)
    raise ValueError('; '.join(errors) + '; GPT-6.1 Terra Mittel über den Codex-Host verwenden')


def g01_baseline(registry: dict) -> str:
    specs = list((ROOT / "docs/specs").glob("*/spec.md"))
    total_fr = total_sc = 0
    for spec in specs:
        text = spec.read_text(encoding="utf-8")
        total_fr += len(set(re.findall(r"FR-(?:\d{3}-)?\d{3}", text)))
        total_sc += len(set(re.findall(r"SC-(?:\d{3}-)?\d{3}", text)))
    status = command_output("git", "status", "--short")
    return f'''# G01: Factory-Probelauf und Bestandsaufnahme

Erstellt: {datetime.now(timezone.utc).isoformat()}

HEAD: `{command_output("git", "rev-parse", "HEAD")}`

Plan-SHA256: `{digest(PLAN)}`

Matrix-SHA256: `{digest(MATRIX)}`

## Automatisch erhobene Befunde

- {len(specs)} Specs, {total_fr} verschiedene specbezogene FR und {total_sc} SC durch ID-Zählung.
- Register: {len(registry['items'])} Einträge; Abhängigkeiten und Zyklen geprüft.
- Die frühere Analyse ist statisch; Zählung und Quellverweise ersetzen keine individuelle Laufzeitvalidierung aller Anforderungen.
- Git-Status zum Zeitpunkt der Erhebung (einschließlich bereits vorhandener Nutzeränderungen):

```text
{status}
```

## ADR-Entwurf: native Umsetzung

Bestehendes Rust-/Slint-Programm und explizite Konstruktion bleiben Ausgangspunkt. Browser-/Electron-Begriffe werden pro Spec auf native Plattformdienste abgebildet. Sichtbares Verhalten und Messziele bleiben Anforderungen; konkrete Bibliotheken und Plattformunterstützung sind vor ihren jeweiligen Slices zu evaluieren. Der Entwurf ist keine Freigabe neuer Abhängigkeiten.

## Konflikt- und Entscheidungsregister

| Thema | Sachstand | Status |
|---|---|---|
| Camelot (Specs 6/25) | Wheel-Nachbarschaft und Halbtonbeispiele fachlich prüfen. | Offen: musikalische Regel und Entscheidung dokumentieren. |
| Cloud (Spec 2) | Last-write-wins und Ziel für Änderungserhalt im Mehrgerätebetrieb zusammen bewerten. | Offen: Konflikt- und Messverfahren. |
| Video-/UI-FPS (7/8/15) | Unterschiedliche Kennzahlen und Lastbedingungen unterscheiden. | Offen: Testumgebungen und Messmethodik. |
| Analysezeiten (3/21) | Gemeinsame Messbedingungen, Cache und Datensatz festlegen. | Offen. |
| Übergänge/Retention (23) | Vollständige Ereigniserfassung und konfigurierbare Begrenzung abgrenzen. | Offen. |
| Musikdienste, DVS, Modelle, Encoder | Zugänge, Lizenzen und tatsächliche technische Verfügbarkeit prüfen. | Offen; eigenes Gate G02. |

## Mess- und Bestandsplan

Für jeden FR/SC sind Quelle, vorhandene Implementierung, Test, Plattform und Nachweis getrennt zu erfassen. Ein annotiertes, lizenziertes Musik-Corpus; Hardware-/OS-/Audiointerface-Matrix; CPU-/Latenz-/Dropout-Verfahren; sowie Kohorten und Beobachtungszeiträume für Nutzer- und Betriebsziele sind noch festzulegen. Bestehende Tests, synthetische Signale und MC4000-Berichte sind keine neue Hardware- oder Hörabnahme.

**G01-Status: offen.** Dies ist ein Probelauf und ein erster überprüfbarer Entwurf. Die tatsächliche Einzelprüfung aller 618 FR/SC und die Entscheidungstore D1–D3/D5/D6 sind nicht abgeschlossen. Es werden keine abhängigen Slices freigegeben.
'''


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["generate", "validate", "next", "prepare", "draft", "check", "start", "block", "unblock", "finish", "pilot-g01"])
    parser.add_argument("slice", nargs="?")
    parser.add_argument('--reason', default='')
    parser.add_argument('--evidence', default='')
    args = parser.parse_args(argv)
    try:
        if args.command == "generate":
            REGISTRY.write_text(json.dumps(extract_registry(), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            print(f"Register geschrieben: {REGISTRY.relative_to(ROOT)}")
            return 0
        registry = load_registry()
        nodes = {x["id"]: x for x in registry["items"]}
        state = load_state()
        if args.command == "validate":
            expected = extract_registry()
            if registry != expected:
                raise ValueError("Register stimmt nicht mit Plan überein")
            print(f"OK: {len(nodes)} Pakete und Slices, gültige Abhängigkeiten")
        elif args.command == "next":
            candidates = [x["id"] for x in registry["items"] if ready(x, nodes, state)]
            print("\n".join(candidates) if candidates else "Kein Slice bereit")
        elif args.command in ("prepare", "draft", "check", "start", "block", "unblock", "finish"):
            if args.slice not in nodes:
                raise ValueError('Unbekannter Slice')
            if args.command == 'unblock':
                if state['slices'].get(args.slice, {}).get('status') != 'blocked':
                    raise ValueError('Slice ist nicht blockiert')
                del state['slices'][args.slice]
                save_state(state)
                print(f'{args.slice}: Blockade aufgehoben; Voraussetzungen erneut prüfen')
                return 0
            if not ready(nodes[args.slice], nodes, state):
                raise ValueError("Slice unbekannt oder Abhängigkeiten nicht erfüllt")
            node = nodes[args.slice]
            if args.command == 'prepare':
                print(json.dumps({"slice": node, "plan": registry["source"], "instructions": "Anforderungen, AGENTS.md, aktuelle Quellen und Tests prüfen; keine Ausführung oder Freigabe impliziert."}, ensure_ascii=False, indent=2))
            elif args.command == 'draft':
                print(draft(node))
            elif args.command == 'start':
                state['slices'][args.slice] = {'status': 'in_progress', 'updated': datetime.now(timezone.utc).isoformat()}
                save_state(state)
                print(f'{args.slice}: in Arbeit')
            elif args.command == 'block':
                if not args.reason.strip():
                    raise ValueError('Blockade benötigt --reason')
                state['slices'][args.slice] = {'status': 'blocked', 'reason': args.reason.strip(), 'updated': datetime.now(timezone.utc).isoformat()}
                save_state(state)
                print(f"{args.slice}: Blockade dokumentiert")
            elif args.command == 'finish':
                if not args.evidence:
                    raise ValueError('Abschluss benötigt --evidence mit Nachweisdatei')
                evidence = (ROOT / args.evidence).resolve()
                if not evidence.is_relative_to(ROOT.resolve()) or not evidence.is_file():
                    raise ValueError('Nachweisdatei muss innerhalb des Repositorys existieren')
                receipt = json.loads(evidence.read_text(encoding='utf-8'))
                if receipt.get('slice') != args.slice or receipt.get('open_blockers') != [] or not receipt.get('checks') or not all(isinstance(c, dict) and c.get('result') == 'passed' and c.get('command') for c in receipt['checks']):
                    raise ValueError('Nachweis benötigt Slice-ID, leere open_blockers und bestandene Prüfungen')
                if args.slice == 'G01' and not all(receipt.get(k) is True for k in ('all_requirements_validated', 'decisions_resolved', 'measurement_plan_defined')):
                    raise ValueError('G01 erfordert Einzelvalidierung, Entscheidungen und Messplan')
                state['slices'][args.slice] = {'status': 'complete', 'evidence': str(evidence.relative_to(ROOT.resolve())), 'evidence_sha256': digest(evidence), 'updated': datetime.now(timezone.utc).isoformat()}
                save_state(state)
                print(f'{args.slice}: Abschluss mit Nachweis dokumentiert; Veröffentlichung erfolgt separat')
            else:
                commands = [[sys.executable, '-m', 'unittest', 'discover', '-s', 'tools/software_factory', '-p', 'test_*.py']]
                if args.slice != 'G01':
                    commands += [['cargo', 'fmt', '--all', '--', '--check'], ['cargo', 'test', '--workspace', '--locked'], ['cargo', 'clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings']]
                for command in commands:
                    print('Prüfe:', ' '.join(command), flush=True)
                    result = subprocess.run(command, cwd=ROOT, check=False)
                    if result.returncode:
                        raise ValueError(f"Prüfung fehlgeschlagen: {command[0]} (Exit {result.returncode})")
                print('Automatische Prüfungen bestanden; dies ist keine Abnahme.')
        else:
            if not ready(nodes["G01"], nodes, state):
                raise ValueError("G01 ist nicht bereit")
            report = ROOT / "docs/specs/G01_FACTORY_PILOT.md"
            if report.exists():
                raise ValueError("Probelaufdatei existiert bereits; vorhandene Ergebnisse werden nicht überschrieben")
            report.write_text(g01_baseline(registry), encoding="utf-8")
            print(f"Probelauf dokumentiert: {report.relative_to(ROOT)}; G01 bleibt offen")
        return 0
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as exc:
        print(f"Factory-Fehler: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
