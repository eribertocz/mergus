# 0055 · Catalog version, freshness and ClickHouse pins for consumers

- Status: accepted
- Context for: phase 0.5 (`contracts/manifest.md`), 1.5 (ClickHouse adapter)

## Context
Accipiter sent five more requests after decision 0054. DuckLake is pinned at `ATTACH`, which
fixes the whole catalog, so a reader needs one snapshot per catalog, not one per table. It also
needs freshness without querying data, the withdrawn signal, the retention of documents, and a
way to read ClickHouse at a known version.

## Decision
1. **Catalog version (0.5):** `catalogs[]` gives one `snapshot_id` per DuckLake catalog. A
   publication commits its gold in one snapshot, and an external catalog is pinned once per run.
   Every table pin of a catalog equals that snapshot. `data_token` stays the snapshot at which
   the table last changed: a catalog snapshot, not a table identifier (MG-REQ-0258).
2. **Withdrawn:** already decided as the marker of MG-REQ-0256 (0.7). It is read from the
   catalog with no remote notice.
3. **Freshness (0.5):** `freshness` {`data_as_of`, `fresh`} per entry. The reader computes the
   age with its own clock; `health.status` is the state at publication (MG-REQ-0259).
4. **Retention:** already decided: documents are never deleted (MG-REQ-0257).
5. **ClickHouse (1.5):** every publication writes an immutable physical table per version and
   swaps the stable name onto it. The pin names the physical table, so ClickHouse reads are
   pinned instead of live, and no check before and after the read is needed (MG-REQ-0244).
   Rejected: a version marker checked before and after the read, because it fails after the
   read instead of preventing the mix.
