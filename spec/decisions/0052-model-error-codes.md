# 0052 · Model validation errors use MG-06xx codes

- Status: accepted
- Answers: #280

## Context
MG-REQ-0654.1 and MG-REQ-0657.2 gave their errors `MG-07xx` codes, but the error-catalog
check (MG-REQ-1335.3) requires a code's area to match the area of the rules it lists. Those
rules are in area 06 (model), so no `MG-07xx` file could list them honestly.

## Decision
The errors of MG-REQ-0654.1, MG-REQ-0657.2 and the new MG-REQ-0661.1 take the next free
`MG-06xx` code. The check is unchanged. `MG-07xx` stays for gate errors of stage 4, which list
the gate rules MG-REQ-0720 and MG-REQ-0722. Open pull requests that take the same next free
code renumber in merge order: the one that merges second renumbers.

## Consequences
#262 uses `MG-06xx` codes. #273 and #275 renumber in the order they merge.
