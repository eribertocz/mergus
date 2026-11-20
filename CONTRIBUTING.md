# Contributing to Mergus

Thank you for your interest in Mergus. This document describes, in short, how a change
reaches the project. The roles behind this workflow are described in `GOVERNANCE.md`.

## Workflow

1. **Issues.** Every change starts from an issue. Implementation issues are filed from the
   specification under `spec/` and state the rules, the files and the acceptance criteria of
   the change.
2. **One pull request per issue.** Work on a branch named `issue-<n>-<short-name>` and open
   exactly one pull request for the issue. Change only the files the issue allows.
3. **Review.** Every implementation pull request is reviewed against its issue and the
   specification. The review ends with a verdict: `ready`, `changes-needed` or
   `spec-defect`. On `changes-needed`, push new commits to the same pull request; do not
   force-push after the review has started.
4. **Merge.** The owner merges pull requests that have green CI and a `ready` verdict.

Before you push, run `just check`. It runs formatting, lints, tests, the boundary check and
the specification check.

If the specification is not enough to do the work without guessing about observable
behavior, open a `spec-defect` issue instead of guessing.

## Sign-off (Developer Certificate of Origin)

Every external contribution must carry a `Signed-off-by` line in each commit, certifying the
[Developer Certificate of Origin](https://developercertificate.org/). Add it with
`git commit -s`. CI checks it.

No contributor license agreement (CLA) is used. Contributions are accepted under the
project's license, the Apache License, Version 2.0 (see `LICENSE`).

## Conduct

Everyone taking part in the project follows the `CODE_OF_CONDUCT.md`.

## Security issues

Do not report vulnerabilities in public issues. Follow `SECURITY.md` instead.
