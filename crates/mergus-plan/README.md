# mergus-plan

Resolve, order and fingerprint the work; produce the plan.

## What it does

- Gives each model a fingerprint: the lowercase hexadecimal SHA-256 of the canonical JSON of
  its code (CRLF turned into LF), its effective option values without origins, each input
  name to the upstream fingerprint or the version the caller gives or `null`, its run
  parameters (`{}` in 0.2), the engine versions the caller gives and its output contract
  (`null` until locked outputs exist) (MG-REQ-0711, MG-REQ-0743).
- Makes one step per selected model: `skip` with `result_available` when its fingerprint is
  among the available results the caller gives, otherwise `compute` with `no_result`; refresh
  `full` (MG-REQ-0712, MG-REQ-0713, MG-REQ-0744.2).
- Cuts the steps of each depth, in name order, into parallel groups of at most
  `max_parallel` (4 by default), numbered from 1 by depth then order (MG-REQ-0716,
  MG-REQ-0744.3).
- Lists each stale model left out of the selection and upstream of a selected one, with the
  selector `+<model>` of the first selected model downstream of it (MG-REQ-0717,
  MG-REQ-0745).
- Returns the plan as a `mergus_plan` document valid against `schemas/mergus_plan/v1/`,
  identified by `plan_id`, the SHA-256 of its canonical JSON without `plan_id`
  (MG-REQ-0714, MG-REQ-0742, MG-REQ-0743). The same inputs give the same bytes
  (MG-REQ-0005, MG-REQ-0715).

## What it does not

- It performs no I/O: the caller reads the project with `mergus-project` and passes the
  models (MG-REQ-0004).
- It executes nothing, stores nothing and writes no file; `mergus plan` (MG-REQ-0746) does.
- It parses no selector: the caller gives the selected model names.
- It plans no segments, boundaries, Polars levels, write strategies or publication contents
  yet (phase 0.2).
