#!/usr/bin/env python3
"""Mergus specification tool (stdlib only). See spec/README.md section 6.

Usage:
  spec_tool.py lint
  spec_tool.py index            write spec/RULES.md and spec/crates.md
  spec_tool.py status           print the status of every rule (never written to the repository)
  spec_tool.py check            fail if generated files are out of date or lint fails
  spec_tool.py context ID|ISSUE...   print the minimal reading for a task
  spec_tool.py issue ID...      print an implementation issue body
"""
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SPEC = ROOT / "spec"
GENERATED = {"RULES.md", "crates.md"}
HEADER_FIELDS = ["id", "title", "crate", "phase", "status", "depends_on"]
STATUSES = {"draft", "stable", "retired"}
KINDS = {"behavior", "process"}
META_KEYS = {"Kind", "Verify", "Phase", "Decision", "Depends", "Status"}
MAX_LINES = 300
RANGES = {
    "kernel": 0, "architecture": 100, "contracts": 200, "project": 300, "extract": 400,
    "storage": 500, "model": 600, "run": 700, "quality": 800, "publish": 900,
    "security": 1000, "observability": 1100, "environments": 1200, "cli": 1300, "ui": 1400,
    "ops": 1500, "distribution": 1600, "governance": 1700,
}
RULE = re.compile(r"^### (MG-REQ-(\d{4})) · (.+)$")
HEADING = re.compile(r"^#{1,3} ")
META = re.compile(r"^- (\w+): (.+)$")
LINK = re.compile(r"\]\(([^)#\s]+)(#[^)]*)?\)")
TEST_FN = re.compile(r"fn (mg_req_(\d{4})_\w+)")


def slug(text):
    text = re.sub(r"[^\w\- ]", "", text.lower())
    return text.replace(" ", "-")


def spec_files():
    # Sort by the POSIX relative path: Windows paths compare without case (MG-REQ-0189).
    for path in sorted(SPEC.rglob("*.md"), key=lambda p: p.relative_to(SPEC).as_posix()):
        rel = path.relative_to(SPEC)
        if path.name in GENERATED and len(rel.parts) == 1:
            continue
        if rel.parts[0] == "decisions":
            continue
        yield path


def parse_header(lines):
    if not lines or lines[0] != "---":
        return None, 0
    header = {}
    for index, line in enumerate(lines[1:], start=1):
        if line == "---":
            return header, index + 1
        key, _, value = line.partition(":")
        value = value.split("#", 1)[0].strip().strip('"')
        if value.startswith("["):
            value = [item.strip() for item in value.strip("[]").split(",") if item.strip()]
        header[key.strip()] = value
    return None, 0


def parse_file(path):
    lines = path.read_text(encoding="utf-8").splitlines()
    header, start = parse_header(lines)
    rules, current, fenced = [], None, False
    for number, line in enumerate(lines[start:], start=start + 1):
        if line.startswith("```"):
            fenced = not fenced
        match = None if fenced else RULE.match(line)
        if match:
            current = {"id": match.group(1), "num": int(match.group(2)), "title": match.group(3),
                       "line": number, "body": [line], "meta": {}, "anchor": slug(line[4:])}
            rules.append(current)
            continue
        if current is not None and not fenced and HEADING.match(line):
            current = None
        if current is not None:
            current["body"].append(line)
            meta = META.match(line)
            if meta and meta.group(1) in META_KEYS:
                current["meta"][meta.group(1)] = meta.group(2).strip()
    for rule in rules:
        while rule["body"] and not rule["body"][-1].strip():
            rule["body"].pop()
    return {"path": path, "rel": path.relative_to(SPEC).as_posix(), "lines": lines,
            "header": header, "rules": rules}


def load():
    return [parse_file(path) for path in spec_files()]


def area_of(doc):
    parts = pathlib.PurePosixPath(doc["rel"]).parts
    return parts[0] if len(parts) > 1 else pathlib.PurePosixPath(doc["rel"]).stem


def rule_phase(doc, rule):
    return rule["meta"].get("Phase") or (doc["header"] or {}).get("phase", "?")


def lint(docs):
    errors, ids, file_ids = [], {}, {}
    for doc in docs:
        rel, header = doc["rel"], doc["header"]
        if header is None:
            errors.append(f"{rel}: missing YAML header")
            continue
        if list(header) != HEADER_FIELDS:
            errors.append(f"{rel}: header fields must be exactly {HEADER_FIELDS} in that order")
        if header.get("status") not in STATUSES:
            errors.append(f"{rel}: status must be one of {sorted(STATUSES)}")
        area = area_of(doc)
        if "/" in rel and header.get("id") != f"{area}-{pathlib.PurePosixPath(rel).stem}":
            errors.append(f"{rel}: id must be '{area}-{pathlib.PurePosixPath(rel).stem}'")
        file_ids[header.get("id")] = rel
        if len(doc["lines"]) > MAX_LINES:
            errors.append(f"{rel}: {len(doc['lines'])} lines, limit {MAX_LINES}")
        for rule in doc["rules"]:
            where = f"{rel}:{rule['line']} {rule['id']}"
            if rule["id"] in ids:
                errors.append(f"{where}: duplicate of {ids[rule['id']]}")
            ids[rule["id"]] = rel
            base = RANGES.get(area)
            if base is None:
                errors.append(f"{where}: rules are not allowed in this file")
            elif not (max(base, 1) <= rule["num"] <= base + 99):
                errors.append(f"{where}: outside the {area} range {base:04d}-{base + 99:04d}")
            kind = rule["meta"].get("Kind")
            if kind not in KINDS:
                errors.append(f"{where}: Kind must be behavior or process")
            verify = rule["meta"].get("Verify", "")
            if not re.match(r"^(test|check|review) `[\w\-]+`$", verify):
                errors.append(f"{where}: Verify must be test|check|review `name`")
            elif kind == "behavior" and verify.startswith("test") and \
                    not verify.split("`")[1].startswith(f"mg_req_{rule['num']:04d}_"):
                errors.append(f"{where}: test name must start with mg_req_{rule['num']:04d}_")
        for target, _ in LINK.findall("\n".join(doc["lines"])):
            if not target.startswith("http") and not (doc["path"].parent / target).exists():
                errors.append(f"{rel}: broken link {target}")
    roadmap = SPEC / "roadmap.md"
    phases = set(re.findall(r"^\| \**(\d+\.\d+)\** \|", roadmap.read_text(encoding="utf-8"), re.M)) \
        if roadmap.exists() else set()
    for doc in docs:
        for rule in doc["rules"]:
            if phases and rule_phase(doc, rule) not in phases:
                errors.append(f"{doc['rel']}:{rule['line']} {rule['id']}: phase "
                              f"{rule_phase(doc, rule)} is not in roadmap.md")
    for doc in docs:
        for dep in (doc["header"] or {}).get("depends_on", []) or []:
            if dep not in ids and dep not in file_ids:
                errors.append(f"{doc['rel']}: depends_on '{dep}' does not exist")
        for rule in doc["rules"]:
            for dep in re.findall(r"MG-REQ-\d{4}", rule["meta"].get("Depends", "")):
                if dep not in ids:
                    errors.append(f"{doc['rel']}:{rule['line']}: Depends {dep} does not exist")
    return errors


def render_rules(docs):
    out = ["# Rules", "", "Generated by `scripts/spec_tool.py index`. Do not edit by hand.", "",
           "| Rule | Title | Kind | Phase | File |", "|---|---|---|---|---|"]
    rows = sorted((rule["id"], rule, doc) for doc in docs for rule in doc["rules"])
    for rule_id, rule, doc in rows:
        out.append(f"| {rule_id} | {rule['title']} | {rule['meta'].get('Kind', '?')} | "
                   f"{rule_phase(doc, rule)} | [{doc['rel']}]({doc['rel']}#{rule['anchor']}) |")
    return "\n".join(out) + "\n"


def render_crates(docs):
    out = ["# Crates", "", "Generated by `scripts/spec_tool.py index`. Do not edit by hand.", "",
           "| Crate | Specification files |", "|---|---|"]
    crates = {}
    for doc in docs:
        crate = (doc["header"] or {}).get("crate", "none")
        crates.setdefault(crate, []).append(f"[{doc['rel']}]({doc['rel']})")
    for crate in sorted(crates):
        out.append(f"| {crate} | {', '.join(crates[crate])} |")
    return "\n".join(out) + "\n"


def found_tests():
    tests = {}
    for path in ROOT.rglob("*.rs"):
        if "target" in path.parts:
            continue
        for name, number in TEST_FN.findall(path.read_text(encoding="utf-8", errors="ignore")):
            tests.setdefault(int(number), []).append(name)
    return tests


def render_status(docs):
    tests = found_tests()
    out = ["# Status", "", "Generated by `scripts/spec_tool.py status`. Do not edit by hand.", "",
           "| Rule | Phase | Kind | Verify | State |", "|---|---|---|---|---|"]
    rows = sorted((rule["id"], rule, doc) for doc in docs for rule in doc["rules"])
    for rule_id, rule, doc in rows:
        verify = rule["meta"].get("Verify", "")
        if rule["meta"].get("Status") == "retired":
            state = "retired"
        elif verify.startswith("test"):
            state = "tested" if rule["num"] in tests else "pending"
        else:
            state = "by " + verify.split(" ")[0]
        out.append(f"| {rule_id} | {rule_phase(doc, rule)} | {rule['meta'].get('Kind', '?')} | "
                   f"{verify} | {state} |")
    return "\n".join(out) + "\n"


def index_by_id(docs):
    return {rule["id"]: (rule, doc) for doc in docs for rule in doc["rules"]}


def issue_ids(argument):
    if re.fullmatch(r"MG-REQ-\d{4}", argument):
        return [argument]
    if argument.isdigit():
        raw = subprocess.run(["gh", "api", f"repos/{{owner}}/{{repo}}/issues/{argument}"],
                             capture_output=True, text=True, check=True).stdout
        return re.findall(r"MG-REQ-\d{4}", json.loads(raw).get("body") or "")
    sys.exit(f"not a rule ID or issue number: {argument}")


def background(doc):
    """Explanatory text of a file: everything after the header except rule blocks."""
    hidden = set()
    for rule in doc["rules"]:
        hidden.update(range(rule["line"], rule["line"] + len(rule["body"])))
    _, start = parse_header(doc["lines"])
    kept = [line for number, line in enumerate(doc["lines"][start:], start=start + 1)
            if number not in hidden]
    text = re.sub(r"\n{3,}", "\n\n", "\n".join(kept)).strip()
    return text.splitlines()


def context(docs, arguments):
    by_id = index_by_id(docs)
    wanted = []
    for argument in arguments:
        wanted += issue_ids(argument)
    deps = []
    for rule_id in wanted:
        if rule_id not in by_id:
            sys.exit(f"unknown rule {rule_id}")
        rule, doc = by_id[rule_id]
        deps += re.findall(r"MG-REQ-\d{4}", rule["meta"].get("Depends", ""))
        deps += [d for d in (doc["header"] or {}).get("depends_on", []) if d.startswith("MG-REQ")]
    kernel = (SPEC / "kernel.md").read_text(encoding="utf-8")
    out = ["# Context", "", "## Kernel (always applies)", "", kernel.split("---", 2)[2].strip(),
           "", "## Background of the files of these rules", ""]
    for doc in dict.fromkeys(by_id[rule_id][1]["rel"] for rule_id in wanted):
        out += [f"<!-- {doc} -->", *background(next(d for d in docs if d["rel"] == doc)), ""]
    out += ["## Rules of this task", ""]
    for rule_id in dict.fromkeys(wanted):
        rule, doc = by_id[rule_id]
        out += [f"<!-- {doc['rel']} -->", *rule["body"], ""]
    extra = [d for d in dict.fromkeys(deps) if d not in wanted and not d.startswith("MG-REQ-00")]
    if extra:
        out += ["## Rules they depend on", ""]
        for rule_id in extra:
            rule, doc = by_id[rule_id]
            out += [f"<!-- {doc['rel']} -->", *rule["body"], ""]
    return "\n".join(out) + "\n"


def issue(docs, ids):
    by_id = index_by_id(docs)
    missing = [i for i in ids if i not in by_id]
    if missing:
        sys.exit(f"unknown rules: {missing}")
    crates = {(by_id[i][1]["header"] or {}).get("crate") for i in ids}
    if len(crates) != 1:
        sys.exit(f"an issue covers one crate; these rules span {sorted(crates)}")
    tests = [by_id[i][0]["meta"]["Verify"] for i in ids]
    out = ["## Rules", *[f"- {i} · {by_id[i][0]['title']}" for i in ids], "",
           "## Crate", crates.pop(), "", "## Read",
           "Run `just spec-context <this issue number>` and read only its output.", "",
           "## Acceptance criteria", *[f"- {t}" for t in tests], "",
           "## Out of scope", "Everything not required by the rules above.", "",
           "## Files", "<!-- one path per line; a path ending in / covers everything under it -->", "",
           "## Depends on", "<!-- issue numbers -->", "", "## Parallel with", "<!-- issue numbers -->"]
    return "\n".join(out) + "\n"


def generated(docs):
    return {SPEC / "RULES.md": render_rules(docs), SPEC / "crates.md": render_crates(docs),
            }


def main(argv):
    if not argv:
        sys.exit(__doc__)
    docs, command = load(), argv[0]
    if command == "lint":
        errors = lint(docs)
        print("\n".join(errors) or "spec lint: ok")
        return 1 if errors else 0
    if command == "index":
        for path, text in generated(docs).items():
            path.write_text(text, encoding="utf-8")
        return 0
    if command == "status":
        print(render_status(docs), end="")
        return 0
    if command == "check":
        errors = lint(docs)
        for path, text in generated(docs).items():
            if not path.exists() or path.read_text(encoding="utf-8") != text:
                errors.append(f"{path.relative_to(ROOT)} is out of date: run just spec-index")
        print("\n".join(errors) or "spec check: ok")
        return 1 if errors else 0
    if command == "context":
        print(context(docs, argv[1:]), end="")
        return 0
    if command == "issue":
        print(issue(docs, argv[1:]), end="")
        return 0
    sys.exit(__doc__)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
