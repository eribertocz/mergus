# Specification
spec-lint:
    python3 -I scripts/spec_tool.py lint

spec-index:
    python3 -I scripts/spec_tool.py index

spec-status:
    python3 -I scripts/spec_tool.py status

spec-check:
    python3 -I scripts/spec_tool.py check

spec-context +targets:
    python3 -I scripts/spec_tool.py context {{targets}}

spec-issue +rules:
    python3 -I scripts/spec_tool.py issue {{rules}}
