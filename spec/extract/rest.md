---
id: extract-rest
title: Declarative REST API sources
crate: mergus-source-rest
phase: "0.8"
status: draft
depends_on: [extract-sources, extract-loads]
---

# REST sources

```toml
# sources/stripe.toml
kind = "rest"
base_url = "https://api.stripe.com/v1"
auth = { bearer = "secret:stripe_key" }
rate = { per_second = 20, per_day = 100000 }

[tables.customers]
path = "/customers"
key = "id"
incremental = "created"
```

Everything runs in Rust: HTTP, authentication and JSON decoding. Python never sees a request
or a credential (MG-REQ-0006).

### MG-REQ-0441 · Authentication methods

REST sources MUST support API key (header or query), bearer token, HTTP basic, OAuth2 client
credentials with automatic refresh, and HMAC request signing. Every credential is a secret
reference.

- Kind: behavior
- Verify: test `mg_req_0441_supported_auth_methods`

### MG-REQ-0442 · Automatic pagination

Mergus MUST detect and follow cursor, offset, page-number, `Link` header and next-token
pagination from the responses. A `pagination` table MAY override detection.

- Kind: behavior
- Verify: test `mg_req_0442_pagination_detected`

### MG-REQ-0443 · JSON to Arrow

Responses MUST be decoded as a stream into Arrow. Nested objects become `struct`, arrays
become `list`; `flatten = <depth>` flattens nested fields to that depth.

- Kind: behavior
- Verify: test `mg_req_0443_nested_json_to_arrow`

### MG-REQ-0444 · Incremental REST

An incremental endpoint MUST send the API's "updated since" parameter when declared, else
load completely and detect changes by `key`. Watermark and overlap rules of
`extract/loads.md` apply.

- Kind: behavior
- Verify: test `mg_req_0444_rest_incremental`

### MG-REQ-0445 · Resume by page

An interrupted REST extraction MUST resume from the last committed page.

- Kind: behavior
- Verify: test `mg_req_0445_rest_resumes_by_page`

### MG-REQ-0446 · Respect the API

Mergus MUST honour `rate`, `Retry-After` and HTTP 429, retry with exponential backoff and
jitter, and open a circuit breaker after five consecutive failures, raising an incident.
It MUST stop before exceeding `per_day`.

- Kind: behavior
- Verify: test `mg_req_0446_rate_limits_and_circuit_breaker`

### MG-REQ-0447 · Generate from OpenAPI or curl

`mergus add source --openapi <url|file>` and `mergus add source --from-curl '<command>'`
MUST write a valid `sources/<name>.toml`, with a `# TODO` comment on every value they could
not determine.

- Kind: behavior
- Verify: test `mg_req_0447_generators_write_valid_source`

### MG-REQ-0448 · source test

`mergus source test <name>` MUST check authentication, the first page, pagination, the
incremental parameter and the inferred schema, and report each as pass or fail.

- Kind: behavior
- Verify: test `mg_req_0448_source_test_reports_each_check`

### MG-REQ-0449 · Record and replay

`mergus source record <name>` MUST save real responses to `fixtures/<name>/` with secrets and
`pii` values removed; tests and the simulator MUST be able to replay them without network.

- Kind: behavior
- Verify: test `mg_req_0449_recorded_fixtures_replay_offline`

### MG-REQ-0450 · Call budget in the plan

`mergus plan` MUST show the estimated number of calls per REST source and refuse to start a
run whose estimate exceeds the remaining daily budget.

- Kind: behavior
- Verify: test `mg_req_0450_plan_refuses_call_budget_overrun`
