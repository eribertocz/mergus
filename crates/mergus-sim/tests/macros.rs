//! MG-REQ-0641: the simulated `Database` answers a macro file with the sandboxed run recorded
//! from the real one.

use mergus_ports::{CatalogMacro, Database, MacroRun, PortError};
use mergus_sim::{DatabaseFaults, SimDatabase};

#[test]
fn mg_req_0641_simulated_database_replays_macro_runs() {
    let database = SimDatabase::new(1, DatabaseFaults::default());
    let script = "CREATE MACRO one() AS 1;";
    let run = MacroRun {
        builtins: vec!["sum".to_owned()],
        refused: None,
        changed: Vec::new(),
        macros: vec![CatalogMacro {
            name: "one".to_owned(),
            table: false,
            definition: "1".to_owned(),
        }],
    };
    database.record_macro_run(script, run.clone());
    assert_eq!(database.run_macros(script).unwrap(), run);
    assert!(matches!(
        database.run_macros("CREATE MACRO two() AS 2;"),
        Err(PortError::NotFound(_))
    ));
}
