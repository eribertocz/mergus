---
id: run-scheduling
title: Freshness, triggers, locking, retries and explanations
crate: mergus-runtime
phase: "0.7"
status: draft
depends_on: [run-stages]
---

# Scheduling

The user declares how fresh data must be, not when to run.

| Trigger | Example | MVP |
|---|---|---|
| Manual | `mergus run` | yes |
| Schedule | Every day at 06:00 in a time zone (cron) | yes |
| New source data | The source watermark advanced | yes (used by `fresh`) |
| File arrival | A new file in a folder or bucket | yes |
| Another project published | Another Mergus manifest changed | after MVP |

### MG-REQ-0731 · Freshness drives runs

A published model with `fresh = <duration>` MUST be run when its age is about to exceed the
target and at least one of its sources has new data. Without new data it MUST NOT run.
Intermediate models run when a published model needs them.

- Kind: behavior
- Verify: test `mg_req_0731_fresh_runs_only_with_new_data`

### MG-REQ-0732 · Freshness breach

A published model older than its `fresh` target MUST raise an incident and be marked in the
manifest's quality state.

- Kind: behavior
- Verify: test `mg_req_0732_freshness_breach_recorded`

### MG-REQ-0733 · Triggers

Triggers MUST be declared in `project.toml` `[triggers]`, MAY be combined with a condition
("at 06:00 only if new data"), and event triggers MUST respect a configurable minimum pause.

- Kind: behavior
- Verify: test `mg_req_0733_triggers_combine_and_pause`

### MG-REQ-0734 · One publisher at a time

Only one run MUST publish to a gold store at a time, through a lock with expiry and
heartbeat. An expired lock of a dead run MAY be taken by another run.

- Kind: behavior
- Verify: test `mg_req_0734_single_publisher_lock`

### MG-REQ-0735 · Triggers during a run coalesce

Triggers received during a run MUST be merged into at most one pending run, which processes
everything new.

- Kind: behavior
- Verify: test `mg_req_0735_triggers_coalesce`

### MG-REQ-0736 · Retries only for transient errors

Transient errors (exit code 3) MUST be retried with exponential backoff and jitter; permanent
errors MUST NOT be retried and MUST alert at once with the fix.

- Kind: behavior
- Verify: test `mg_req_0736_only_transient_errors_retried`

### MG-REQ-0737 · Emergency brake

After N consecutive failed runs (default 3), `mergus schedule` MUST pause the project's
schedule, raise an incident with the reason, and resume only with `mergus schedule resume`.

- Kind: behavior
- Verify: test `mg_req_0737_schedule_pauses_after_failures`

### MG-REQ-0738 · No catch-up storm

After the scheduler was down, it MUST run once and process everything pending, never once per
missed schedule.

- Kind: behavior
- Verify: test `mg_req_0738_single_run_after_downtime`

### MG-REQ-0739 · Time limits

A maximum duration MUST be configurable per run and per step; exceeding it cancels with exit
code 7.

- Kind: behavior
- Verify: test `mg_req_0739_time_limit_cancels`

### MG-REQ-0740 · why

`mergus why <model>` MUST explain the last decision about the model (ran, skipped, waiting)
with the facts behind it: freshness, source data times, fingerprint changes.

- Kind: behavior
- Verify: test `mg_req_0740_why_explains_decision`

### MG-REQ-0741 · External orchestrators

`mergus run` MUST behave identically under cron or any external orchestrator, relying only on
its exit code and `--json` events.

- Kind: behavior
- Verify: test `mg_req_0741_run_is_orchestrator_agnostic`
