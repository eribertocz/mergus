---
id: glossary
title: Terms used across the specification, CLI, UI and documentation
crate: none
phase: "0.1"
status: draft
depends_on: []
---

# Glossary

Every term below has exactly this meaning everywhere: specification, code, CLI output, UI and
documentation. Synonyms in the right column are forbidden in user-facing text.

| Term | Meaning | Do not say |
|---|---|---|
| **project** | A directory with `project.toml`, `models/`, and optionally `sources/`, `macros/`, `lib/`, `seeds/` | repo, workspace |
| **source** | A declared origin of data: a database, a set of files or a REST API | connector, input |
| **adapter** | Code that implements the storage contract for one storage format | driver, plugin |
| **extraction** | Copying data from a source into raw | ingestion, sync |
| **raw** | The encrypted, append-only layer holding extracted data as received | bronze, landing |
| **changes table** | `raw.<source>__<table>__changes`: the row-level changes captured for a table | delta table, CDC log |
| **model** | One `.sql` or `.py` file that defines one table | transformation, job |
| **step** | One unit inside a Python model: a function returning SQL or a LazyFrame | node, task |
| **intermediate table** | Any model table not under `models/gold/` | silver, staging |
| **gold table** | A model under `models/gold/<schema>/<table>`, published as `gold.<schema>.<table>` | mart, published model |
| **macro** | A DuckDB `CREATE MACRO` in `macros/*.sql` | function, UDF |
| **seed** | A CSV file in `seeds/` loaded as a raw table | fixture, reference file |
| **history table** | A model with `refresh = "history"` keeping every version of each key (SCD type 2) | snapshot, SCD table |
| **watermark** | The highest value of the incremental column already extracted | cursor, bookmark |
| **overlap** | The time window re-read below the watermark to catch late rows | lookback (that is for models) |
| **lookback** | The window of periods a model recomputes to absorb late data | overlap (that is for sources) |
| **period** | A slice of a table by time, the unit of incremental work and of tokens | partition, chunk |
| **token** | The identifier of the data version of one period | hash, etag |
| **fingerprint** | The hash of everything that defines a model's output: code, config, inputs | checksum |
| **plan** | The ordered, fingerprinted list of work a run will do | DAG, graph |
| **run** | One execution of a plan | job, build |
| **gate** | A check that must pass before data moves forward | guard, barrier |
| **test** | A data check declared for a model or source | assertion, expectation |
| **contract** | A versioned document that crosses a module boundary; also a model's locked output schema | interface, API |
| **manifest** | The document describing one complete publication | snapshot, release |
| **pointer** | The reference `envs/<env>/latest` naming the current manifest of an environment | head, tag |
| **publication** | Moving the pointer to a new manifest | deploy, release |
| **environment** | An isolated set of published data and pointer, such as `dev` or `prod` | target, stage |
| **catalog** | The schema and state documents Mergus produces for Menura | metadata, metastore |
| **journal** | The hash-chained, append-only log of everything a run did | audit log, history |
| **ops tables** | The `ops.*` tables describing runs, tests, freshness and incidents | metadata tables |
| **incident** | A recorded failure or violation that needs attention | alert, issue |
| **secret reference** | `secret:<name>`, pointing to a secret resolved by `mergus-security` | credential, password |
| **writer identity** | The only identity allowed to write gold | service account |
| **owner** | The human maintainer of the repository | admin, maintainer |
