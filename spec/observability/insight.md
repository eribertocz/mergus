---
id: observability-insight
title: Health status, lineage, audit in time, alerts and open standards
crate: mergus-ops
phase: "0.7"
status: draft
depends_on: [observability-journal]
---

# Insight

### MG-REQ-1121 · Health per table

`mergus status` MUST show each published table as healthy, warning or failing, combining
freshness, quality and last run, with the reason. The same state MUST be in the manifest.

- Kind: behavior
- Verify: test `mg_req_1121_status_combines_three_signals`

### MG-REQ-1122 · Versioned lineage

`mergus lineage <table[.column]> --upstream|--downstream` MUST show the lineage with the
exact load or version each column came from. Column lineage is required for SQL steps in the
MVP; Polars steps have table lineage in the MVP and column lineage later.

- Kind: behavior
- Verify: test `mg_req_1122_lineage_with_versions`

### MG-REQ-1123 · Audit in time

`mergus audit <table> --at <time>` MUST state which manifest, version, quality and source
loads the table had at that time, within retention.

- Kind: behavior
- Verify: test `mg_req_1123_audit_at_time`

### MG-REQ-1124 · Alert types and channel

Mergus MUST alert on: failed run, publication blocked by quality, freshness breach, volume
anomaly, emergency brake, abnormal duration. The MVP channel is an outgoing webhook.

- Kind: behavior
- Verify: test `mg_req_1124_alert_types_sent_by_webhook`

### MG-REQ-1125 · One incident, one alert

Mergus MUST walk lineage up to the first failing point and send one alert with the probable
cause, grouping the alerts of affected tables and repeats into it. The alert carries the
model `owner` and MUST resolve itself when a later run passes.

- Kind: behavior
- Verify: test `mg_req_1125_alerts_grouped_by_root_cause`

### MG-REQ-1126 · OpenTelemetry

A run MUST be a trace and each stage, model and segment a span, following OpenTelemetry
semantic conventions. Export by OTLP or to local files is optional; when the destination
fails, the run continues and the journal keeps everything.

- Kind: behavior
- Verify: test `mg_req_1126_otel_trace_shape`

### MG-REQ-1127 · OpenLineage

Mergus MUST emit OpenLineage run events (start, complete, fail) with dataset schemas and
column lineage, to a file or an HTTP endpoint, only when configured.

- Kind: behavior
- Verify: test `mg_req_1127_openlineage_events`
