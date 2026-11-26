//! MG-REQ-0289: the kit passes a conforming in-memory adapter, and fails adapters that break
//! one rule each: the check named in the report is the one that breaks.

use std::sync::{Arc, Mutex};

use arrow::array::{ArrayRef, AsArray, Int64Array, RecordBatch, RecordBatchIterator};
use arrow::datatypes::{DataType, Field, Int64Type, Schema};
use mergus_adapter::{
    AdapterError, BatchStream, Capabilities, PinRecipe, Retention, StorageAdapter, VersionRef,
    Versioning, WriteMode,
};
use mergus_certify::{Faults, Harness, Status, certify};

/// The one rule a flawed adapter breaks.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
enum Flaw {
    None,
    /// `write` changes the newest version in place.
    VisibleWrite,
    /// `commit` publishes a new version, then its rows one by one.
    PartialCommit,
    /// `expire` deletes every version but the newest.
    ExpireLive,
}

/// One version: its rows, `None` once expired. Versions take no time: every one is inside
/// any rollback window longer than zero.
#[cfg(test)]
type Version = Option<Vec<i64>>;

/// The versions of one store, shared by its clients.
#[cfg(test)]
type Lake = Arc<Mutex<Vec<Version>>>;

#[cfg(test)]
struct Memory {
    lake: Lake,
    faults: Arc<Faults>,
    flaw: Flaw,
    staged: Vec<(WriteMode, Vec<i64>)>,
}

#[cfg(test)]
impl Memory {
    /// One storage call: fails once the kit crashed the client.
    fn call(&self) -> Result<(), AdapterError> {
        if self.faults.crashed() {
            return Err(AdapterError::Storage("simulated crash".into()));
        }
        Ok(())
    }

    fn index(version: &VersionRef) -> Option<usize> {
        version.as_str().parse().ok()
    }
}

#[cfg(test)]
fn apply(mode: WriteMode, before: &mut Vec<i64>, rows: &[i64]) {
    if mode == WriteMode::Replace {
        before.clear();
    }
    before.extend_from_slice(rows);
}

#[cfg(test)]
fn ids(data: BatchStream) -> Result<Vec<i64>, AdapterError> {
    let mut ids = Vec::new();
    for batch in data {
        ids.extend(batch?.column(0).as_primitive::<Int64Type>().values().iter());
    }
    Ok(ids)
}

#[cfg(test)]
impl StorageAdapter for Memory {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            versioning: Versioning::Native,
            multi_table_transactions: false,
            concurrent_clients: true,
            exact_types: vec![DataType::Int64],
            write_modes: vec![WriteMode::Append, WriteMode::Replace],
        }
    }

    fn write(&mut self, _: &str, mode: WriteMode, data: BatchStream) -> Result<(), AdapterError> {
        self.call()?;
        let rows = ids(data)?;
        if self.flaw == Flaw::VisibleWrite {
            let mut lake = self.lake.lock().unwrap();
            if let Some(Some(newest)) = lake.last_mut() {
                apply(mode, newest, &rows);
            }
        }
        self.staged.push((mode, rows));
        Ok(())
    }

    fn commit(&mut self) -> Result<VersionRef, AdapterError> {
        let staged = std::mem::take(&mut self.staged);
        if self.flaw == Flaw::PartialCommit {
            // A new version first, then its rows one storage call at a time.
            self.call()?;
            let mut lake = self.lake.lock().unwrap();
            let newest = lake.iter().rev().find_map(Clone::clone);
            lake.push(newest.or_else(|| Some(Vec::new())));
            let index = lake.len() - 1;
            drop(lake);
            for (mode, rows) in staged {
                if mode == WriteMode::Replace {
                    self.lake.lock().unwrap()[index] = Some(Vec::new());
                }
                for row in rows {
                    self.call()?;
                    if let Some(rows) = &mut self.lake.lock().unwrap()[index] {
                        rows.push(row);
                    }
                }
            }
            return Ok(VersionRef::new(index.to_string()));
        }
        self.call()?;
        let mut lake = self.lake.lock().unwrap();
        let mut rows = lake.iter().rev().find_map(Clone::clone).unwrap_or_default();
        for (mode, written) in staged {
            apply(mode, &mut rows, &written);
        }
        lake.push(Some(rows));
        Ok(VersionRef::new((lake.len() - 1).to_string()))
    }

    fn pin_recipe(&self, _: &str, version: &VersionRef) -> Result<PinRecipe, AdapterError> {
        self.call()?;
        let known =
            Self::index(version).is_some_and(|index| index < self.lake.lock().unwrap().len());
        if known {
            Ok(PinRecipe::new(format!(
                "memory version {}",
                version.as_str()
            )))
        } else {
            Err(AdapterError::Storage("unknown version".into()))
        }
    }

    fn read_at(&self, table: &str, version: &VersionRef) -> Result<BatchStream, AdapterError> {
        self.call()?;
        let lake = self.lake.lock().unwrap();
        let rows = Self::index(version)
            .and_then(|index| lake.get(index))
            .and_then(Clone::clone)
            .ok_or_else(|| AdapterError::VersionUnavailable {
                table: table.to_owned(),
                version: version.clone(),
            })?;
        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, true)]));
        let column: ArrayRef = Arc::new(Int64Array::from(rows));
        let batch = RecordBatch::try_new(Arc::clone(&schema), vec![column])?;
        Ok(Box::new(RecordBatchIterator::new([Ok(batch)], schema)))
    }

    fn expire(&mut self, retention: &Retention) -> Result<(), AdapterError> {
        self.call()?;
        let mut lake = self.lake.lock().unwrap();
        let newest = lake.len().saturating_sub(1);
        for (index, rows) in lake.iter_mut().enumerate() {
            let live = retention.live.contains(&VersionRef::new(index.to_string()));
            let in_window = !retention.rollback_window.is_zero();
            let kept = if self.flaw == Flaw::ExpireLive {
                index == newest
            } else {
                live || in_window
            };
            if !kept {
                *rows = None;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
struct MemoryHarness(Flaw);

#[cfg(test)]
impl Harness for MemoryHarness {
    type Store = Lake;

    fn store(&self) -> Result<Lake, AdapterError> {
        Ok(Arc::default())
    }

    fn open(
        &self,
        lake: &Lake,
        faults: Arc<Faults>,
    ) -> Result<Box<dyn StorageAdapter>, AdapterError> {
        Ok(Box::new(Memory {
            lake: Arc::clone(lake),
            faults,
            flaw: self.0,
            staged: Vec::new(),
        }))
    }
}

/// Returns the names of the checks the adapter with `flaw` fails.
#[cfg(test)]
fn failed(flaw: Flaw) -> Vec<&'static str> {
    certify(&MemoryHarness(flaw))
        .failures()
        .map(|outcome| outcome.check)
        .collect()
}

#[test]
fn mg_req_0289_kit_passes_a_conforming_adapter() {
    let report = certify(&MemoryHarness(Flaw::None));
    assert!(report.passed(), "{report}");
    let not_applicable: Vec<&str> = report
        .outcomes()
        .iter()
        .filter(|outcome| matches!(outcome.status, Status::NotApplicable(_)))
        .map(|outcome| outcome.check)
        .collect();
    assert_eq!(
        not_applicable,
        ["write replace period", "write merge by key"],
        "{report}"
    );
}

#[test]
fn mg_req_0289_kit_fails_a_write_visible_before_commit() {
    let failed = failed(Flaw::VisibleWrite);
    assert!(
        failed.contains(&"write invisible before commit"),
        "{failed:?}"
    );
}

#[test]
fn mg_req_0289_kit_fails_a_partial_commit_at_a_crash() {
    // Concurrent clients may also see the half-filled version, so other checks can fail too.
    let failed = failed(Flaw::PartialCommit);
    assert!(
        failed.contains(&"crash between write and commit"),
        "{failed:?}"
    );
}

#[test]
fn mg_req_0289_kit_fails_an_expire_of_live_versions() {
    let failed = failed(Flaw::ExpireLive);
    assert!(failed.contains(&"expire keeps live versions"), "{failed:?}");
    assert!(
        failed.contains(&"expire keeps the rollback window"),
        "{failed:?}"
    );
}
