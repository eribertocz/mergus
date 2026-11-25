# 0039 · What mergus secret set reads and how it fails

- Status: accepted
- Date: 2026-11-25
- Answers: #185

## Context

MG-REQ-1007 said where `mergus secret set` stores a value but not what the value is, whether
a terminal gets a prompt, or which codes its failures carry.

## Decision

- The value is all of standard input minus one trailing line ending, so multi-line secrets
  such as PEM keys work and `printf 'pw\n' | mergus secret set crm` stores `pw`.
- On a terminal, one prompt on standard error, asked once; the value ends at the first line
  ending. No confirmation prompt: the user can run the command again.
- An empty value is refused with `MG-1001`. Unlike an empty environment variable
  (MG-REQ-1001.1), an empty keychain entry is almost always a mistake.
- A keychain that cannot be written, such as a headless Linux machine without a Secret
  Service, fails with `MG-1002`. Mergus never falls back to another store.
- Names use `MG-1305`, extra arguments `MG-1301`, as the other commands; no diagnostic
  contains the value.

## Consequences

- `MG-1001` and `MG-1002` are the first codes of the security area.
- Reading standard input stays in `main`, as for `mergus init`, and reaches the command as a
  reader, so tests need no terminal.
