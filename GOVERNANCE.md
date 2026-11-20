# Governance

## Roles

| Role | Does |
|---|---|
| Owner | Merges pull requests, creates release tags and decides owner-level questions (issues labelled `needs-owner`). |
| Specifier | Maintains the specification under `spec/`, files implementation issues, answers `spec-defect` issues and reviews implementation pull requests. |
| Implementer | Implements one issue in one pull request. |
| Release preparer | Opens the release pull request of a roadmap phase. |
| CI | Runs the blocking checks and publishes after a release tag. |

Only the owner merges pull requests, creates release tags and decides owner-level questions.
Owner-level questions are those that are irreversible, change a released public contract,
affect security or privacy, touch licensing, trademarks or cost, or contradict an approved
decision.

## Workflow

The workflow for issues, pull requests and review is described in `CONTRIBUTING.md`.

## Related documents

- `LICENSE` and `LICENSE-PLEDGE.md`: the license and the pledge never to relicense.
- `TRADEMARKS.md`: the use of the Mergus name and logo.
- `CODE_OF_CONDUCT.md`: the code of conduct.
- `SECURITY.md`: how to report a vulnerability.
