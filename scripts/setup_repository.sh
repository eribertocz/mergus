#!/usr/bin/env bash
# Configure the Mergus GitHub repository (MG-REQ-1725, MG-REQ-1720, MG-REQ-1724).
#
# Only the owner runs this script. Sessions never run it.

set -euo pipefail

readonly MAIN_RULESET="main"
readonly TAG_RULESET="release-tags"

# App id of GitHub Actions. Pinning each required check to it means a commit status or a
# check run posted by anyone else, such as a session using the owner's token, never counts.
readonly GITHUB_ACTIONS_APP_ID=15368

# Blocking guards of MG-REQ-1724. `language` is a warning only, so it is not required.
readonly REQUIRED_CHECKS=(
  scope
  session-identity
  independent-review
  issue-format
  pr-format
  commit-format
  spec-check
)

# Crate names of spec/architecture/crates.md section 2.
readonly CRATES=(
  mergus-contracts mergus-ports mergus-project mergus-plan mergus-engine-duckdb
  mergus-engine-polars mergus-source mergus-source-rest mergus-adapter
  mergus-adapter-ducklake mergus-catalog mergus-quality mergus-ops mergus-security
  mergus-menura mergus-fetch mergus-runtime mergus-cli mergus-py mergus-certify
  mergus-sim mergus-testkit
)

usage() {
  cat <<'EOF'
Usage: scripts/setup_repository.sh [--repo OWNER/NAME] <command>

Configures the GitHub repository with the GitHub CLI (`gh`, authenticated as the owner).
Every command is safe to run again: it updates what exists and creates nothing twice.

Commands:
  setup            Step 1. Run once, before any other issue is filed.
                   - Ruleset "main" on branch main: pull request required,
                     force pushes and deletion forbidden. No required checks yet.
                     If the ruleset already requires the checks, they are kept.
                   - Ruleset "release-tags" on tags v*: only repository admins (the
                     owner) may create, update or delete them.
                   - Labels: spec, impl, release, spec-defect, needs-owner and one
                     label per crate.

  required-checks  Step 2. Run once the CI guards exist and report on pull requests.
                   Adds to ruleset "main" the required status checks:
                   scope, session-identity, independent-review, issue-format,
                   pr-format, commit-format, spec-check.
                   (`language` is a warning only and is not required.)
                   Each check counts only when GitHub Actions reports it, so every
                   guard must run as a GitHub Actions job named exactly as above.

  help             Show this text.

Options:
  --repo OWNER/NAME  Repository to configure. Defaults to the repository of the
                     current directory's git remote.
EOF
}

die() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

resolve_repo() {
  if [[ -n "${REPO:-}" ]]; then
    printf '%s\n' "$REPO"
    return
  fi
  gh api 'repos/{owner}/{repo}' --jq '.full_name'
}

# Print the id of the ruleset named $1, or nothing if it does not exist.
ruleset_id() {
  gh api --paginate "repos/$REPO/rulesets?includes_parents=false" \
    --jq ".[] | select(.name == \"$1\") | .id"
}

# Create the ruleset named $1 with the JSON $2, or replace it if it exists.
upsert_ruleset() {
  local name="$1" json="$2" id
  id="$(ruleset_id "$name")"
  if [[ -n "$id" ]]; then
    gh api --method PUT "repos/$REPO/rulesets/$id" --input - --silent <<<"$json"
    printf 'updated ruleset %s\n' "$name"
  else
    gh api --method POST "repos/$REPO/rulesets" --input - --silent <<<"$json"
    printf 'created ruleset %s\n' "$name"
  fi
}

# Print "yes" if ruleset "main" exists and already requires status checks.
main_has_required_checks() {
  local id count
  id="$(ruleset_id "$MAIN_RULESET")"
  if [[ -z "$id" ]]; then
    return
  fi
  count="$(gh api "repos/$REPO/rulesets/$id" \
    --jq '[.rules[] | select(.type == "required_status_checks")] | length')"
  if [[ "$count" != "0" ]]; then
    printf 'yes\n'
  fi
}

required_checks_rule() {
  local contexts="" check
  for check in "${REQUIRED_CHECKS[@]}"; do
    contexts+="${contexts:+,}{\"context\":\"$check\",\"integration_id\":$GITHUB_ACTIONS_APP_ID}"
  done
  cat <<EOF
,{"type":"required_status_checks","parameters":{
  "strict_required_status_checks_policy":false,
  "do_not_enforce_on_create":false,
  "required_status_checks":[$contexts]}}
EOF
}

# Print the JSON of ruleset "main". With $1 = yes, it requires the status checks.
main_ruleset_json() {
  local checks=""
  if [[ "$1" == "yes" ]]; then
    checks="$(required_checks_rule)"
  fi
  cat <<EOF
{
  "name": "$MAIN_RULESET",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": {"ref_name": {"include": ["refs/heads/main"], "exclude": []}},
  "rules": [
    {"type": "deletion"},
    {"type": "non_fast_forward"},
    {"type": "pull_request", "parameters": {
      "required_approving_review_count": 0,
      "dismiss_stale_reviews_on_push": false,
      "require_code_owner_review": false,
      "require_last_push_approval": false,
      "required_review_thread_resolution": false}}$checks
  ]
}
EOF
}

# Repository role id 5 is "admin"; in a personal repository that is the owner.
tag_ruleset_json() {
  cat <<EOF
{
  "name": "$TAG_RULESET",
  "target": "tag",
  "enforcement": "active",
  "bypass_actors": [
    {"actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always"}
  ],
  "conditions": {"ref_name": {"include": ["refs/tags/v*"], "exclude": []}},
  "rules": [
    {"type": "creation"},
    {"type": "update"},
    {"type": "deletion"},
    {"type": "non_fast_forward"}
  ]
}
EOF
}

# Create the label $1 with color $2 and description $3, or update it if it exists.
upsert_label() {
  local name="$1" color="$2" description="$3"
  if gh api "repos/$REPO/labels/$name" --silent 2>/dev/null; then
    gh api --method PATCH "repos/$REPO/labels/$name" --silent \
      -f color="$color" -f description="$description"
    printf 'updated label %s\n' "$name"
  else
    gh api --method POST "repos/$REPO/labels" --silent \
      -f name="$name" -f color="$color" -f description="$description"
    printf 'created label %s\n' "$name"
  fi
}

setup_labels() {
  local crate
  upsert_label spec "1d76db" "Specification change"
  upsert_label impl "0e8a16" "Implementation of specification rules"
  upsert_label release "5319e7" "Release preparation"
  upsert_label spec-defect "d93f0b" "The specification is missing, ambiguous or wrong"
  upsert_label needs-owner "fbca04" "Needs a decision from the owner"
  for crate in "${CRATES[@]}"; do
    upsert_label "$crate" "c5def5" "Crate $crate"
  done
}

cmd_setup() {
  upsert_ruleset "$MAIN_RULESET" "$(main_ruleset_json "$(main_has_required_checks)")"
  upsert_ruleset "$TAG_RULESET" "$(tag_ruleset_json)"
  setup_labels
}

cmd_required_checks() {
  upsert_ruleset "$MAIN_RULESET" "$(main_ruleset_json yes)"
}

main() {
  local command=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --repo)
        [[ $# -ge 2 ]] || die "--repo needs a value"
        REPO="$2"
        shift 2
        ;;
      -h | --help | help)
        usage
        return
        ;;
      -*) die "unknown option: $1" ;;
      *)
        [[ -z "$command" ]] || die "only one command is allowed"
        command="$1"
        shift
        ;;
    esac
  done

  command -v gh >/dev/null || die "the GitHub CLI (gh) is required"
  case "$command" in
    setup | required-checks) ;;
    "")
      usage >&2
      exit 2
      ;;
    *) die "unknown command: $command (see --help)" ;;
  esac

  REPO="$(resolve_repo)"
  [[ -n "$REPO" ]] || die "cannot resolve the repository; pass --repo OWNER/NAME"
  printf 'repository: %s\n' "$REPO"

  case "$command" in
    setup) cmd_setup ;;
    required-checks) cmd_required_checks ;;
  esac
}

main "$@"
