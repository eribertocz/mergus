---
name: prepare-release
description: Open the release pull request of a roadmap phase when its gates pass. Use when asked to prepare a release.
---

# Prepare a release

1. Read the phase in `spec/roadmap.md` and the output of `just spec-status`.
2. Check every gate of the phase. If one fails, report which and stop.
3. On `release/<version>`, bump the version and generate `CHANGELOG.md`. Change nothing else (MG-REQ-1711).
4. Open the pull request listing each gate and its evidence.
5. Stop. The owner merges and pushes the tag.
