"""Print, as JSON, the Mergus tree of the Python file read from standard input (MG-REQ-0611).

`RealPython` runs this script with `python -I -S -c`. The file is only parsed with `ast`: it is
never compiled to bytecode, imported or executed, so nothing it imports runs. The shape of
the output is documented on `PythonTree` in `mergus-ports`.
"""

import ast
import json
import sys

VERSION = 1

PARAMETER_KINDS = (
    ("posonlyargs", "positional_only"),
    ("args", "positional"),
    ("vararg", "var_positional"),
    ("kwonlyargs", "keyword_only"),
    ("kwarg", "var_keyword"),
)

SCOPES = (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef, ast.Lambda)


class Positions:
    """Converts `ast` positions to 1-based lines and 1-based columns in characters."""

    def __init__(self, text):
        self.lines = text.encode("utf-8").splitlines()

    def of(self, node):
        line = node.lineno
        offset = node.col_offset
        if 0 < line <= len(self.lines):
            column = len(self.lines[line - 1][:offset].decode("utf-8", "replace")) + 1
        else:
            column = offset + 1
        return {"line": line, "column": column}


def parameters(arguments, positions):
    found = []
    for field, kind in PARAMETER_KINDS:
        value = getattr(arguments, field)
        for argument in value if isinstance(value, list) else [value]:
            if argument is not None:
                found.append({"name": argument.arg, "kind": kind, **positions.of(argument)})
    found.sort(key=lambda parameter: (parameter["line"], parameter["column"]))
    return found


def returns_of(function):
    found = []
    pending = list(ast.iter_child_nodes(function))
    while pending:
        node = pending.pop(0)
        if isinstance(node, ast.Return):
            found.append(node)
        if not isinstance(node, SCOPES):
            pending.extend(ast.iter_child_nodes(node))
    found.sort(key=lambda node: (node.lineno, node.col_offset))
    return found


def returned(node, positions):
    value = node.value
    if value is None or (isinstance(value, ast.Constant) and value.value is None):
        shape = {"kind": "none"}
    elif isinstance(value, ast.Constant) and isinstance(value.value, str):
        shape = {"kind": "string", "value": value.value}
    else:
        shape = {"kind": "expression", "node": type(value).__name__}
    return {**shape, **positions.of(node)}


def function(node, positions):
    return {
        "name": node.name,
        "async": isinstance(node, ast.AsyncFunctionDef),
        "parameters": parameters(node.args, positions),
        "returns": [returned(each, positions) for each in returns_of(node)],
        **positions.of(node),
    }


def imports(tree, positions):
    found = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                found.append(
                    {"module": alias.name, "name": None, "alias": alias.asname, "level": 0,
                     **positions.of(node)}
                )
        elif isinstance(node, ast.ImportFrom):
            for alias in node.names:
                found.append(
                    {"module": node.module, "name": alias.name, "alias": alias.asname,
                     "level": node.level, **positions.of(node)}
                )
    found.sort(key=lambda entry: (entry["line"], entry["column"]))
    return found


def chain(node):
    """The segments of an expression, from its root to `node` itself."""
    if isinstance(node, ast.Name):
        return [{"kind": "name", "name": node.id}]
    if isinstance(node, ast.Attribute):
        return chain(node.value) + [{"kind": "attribute", "name": node.attr}]
    if isinstance(node, ast.Call):
        return chain(node.func) + [{"kind": "call"}]
    if isinstance(node, ast.Subscript):
        return chain(node.value) + [{"kind": "subscript"}]
    return [{"kind": "other", "node": type(node).__name__}]


def call(node, positions):
    func = node.func
    if isinstance(func, ast.Name):
        name, receiver = func.id, []
    elif isinstance(func, ast.Attribute):
        name, receiver = func.attr, chain(func.value)
    else:
        name, receiver = None, chain(func)
    return {
        "name": name,
        "receiver": receiver,
        "keywords": [keyword.arg for keyword in node.keywords],
        **positions.of(node),
    }


class Calls(ast.NodeVisitor):
    """Every call, in the order of the tree: a call before the calls inside it."""

    def __init__(self, positions):
        self.positions = positions
        self.found = []

    def visit_Call(self, node):
        self.found.append(call(node, self.positions))
        self.generic_visit(node)


def tree_of(text):
    tree = ast.parse(text, filename="<model>", mode="exec")
    positions = Positions(text)
    calls = Calls(positions)
    calls.visit(tree)
    return {
        "version": VERSION,
        "functions": [
            function(node, positions)
            for node in tree.body
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
        ],
        "imports": imports(tree, positions),
        "calls": calls.found,
    }


def error_of(error):
    line = getattr(error, "lineno", None)
    column = getattr(error, "offset", None)
    message = getattr(error, "msg", None) or str(error) or type(error).__name__
    return {"version": VERSION, "error": {"message": message, "line": line, "column": column}}


def main():
    text = sys.stdin.buffer.read().decode("utf-8")
    if text.startswith("﻿"):
        text = text[1:]
    try:
        document = tree_of(text)
    except (SyntaxError, ValueError, RecursionError, MemoryError) as error:
        document = error_of(error)
    output = json.dumps(document, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
    sys.stdout.buffer.write(output.encode("ascii"))


main()
