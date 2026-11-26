//! Tests of the storage profiles of each zone (MG-REQ-0501, MG-REQ-0521).

use std::path::Path;

use mergus_ports::{FileSystem, PortError};
use mergus_project::{LayoutError, read_configuration};
use mergus_runtime::{
    Catalog, Clients, Profile, StorageProblem, Zone, check_storage, zone_storage,
};
use mergus_sim::SimFileSystem;

const ROOT: &str = "/project";

/// Writes `project` as the `project.toml` of a project at `ROOT`.
fn project(project: &str) -> Result<SimFileSystem, PortError> {
    let fs = SimFileSystem::new();
    let root = Path::new(ROOT);
    fs.create_dir_all(root)?;
    fs.write(&root.join("project.toml"), project.as_bytes())?;
    Ok(fs)
}

/// The codes of the `mergus-project` problems of `text`, or the zones it configures.
fn zones(text: &str) -> Result<Vec<mergus_runtime::ZoneStorage>, Vec<String>> {
    let fs = project(text).map_err(|error| vec![error.to_string()])?;
    match read_configuration(&fs, Path::new(ROOT)) {
        Ok(configuration) => Ok(zone_storage(&configuration)),
        Err(LayoutError::Invalid(problems)) => Err(problems
            .iter()
            .map(|problem| format!("{} {}", problem.code(), problem.key().unwrap_or_default()))
            .collect()),
        Err(other) => Err(vec![other.to_string()]),
    }
}

/// The problems `check_storage` finds in `text`, a valid `project.toml`; any other failure
/// as its message.
fn try_problems(text: &str) -> Result<Vec<StorageProblem>, String> {
    let fs = project(text).map_err(|error| error.to_string())?;
    let root = Path::new(ROOT);
    let configuration = read_configuration(&fs, root).map_err(|error| error.to_string())?;
    check_storage(&fs, root, &configuration).map_err(|error| error.to_string())
}

#[test]
fn mg_req_0521_profile_needs_no_more_keys() {
    // MG-REQ-0521.1: `local` and `shared` need only `location`; `cloud` adds `catalog`.
    let found = zones(
        "name = \"sales\"\n\
         [raw]\nprofile = \"local\"\nlocation = \"lake/raw\"\n\
         [gold]\nprofile = \"shared\"\nlocation = \"/srv/gold/\"\n\
         [ops]\nprofile = \"cloud\"\nlocation = \"s3://ops\"\ncatalog = \"secret:ops_catalog\"\n",
    )
    .unwrap();
    let kinds: Vec<(Zone, Profile)> = found.iter().map(|zone| (zone.zone, zone.profile)).collect();
    assert_eq!(
        kinds,
        [
            (Zone::Raw, Profile::Local),
            (Zone::Gold, Profile::Shared),
            (Zone::Ops, Profile::Cloud)
        ]
    );
    let [raw, gold, ops] = found.as_slice() else {
        panic!("three zones expected");
    };
    assert_eq!(
        raw.catalog,
        Catalog::DuckDb {
            file: "lake/raw/catalog.ducklake".to_owned()
        }
    );
    assert_eq!(
        raw.catalog.attach(None).as_deref(),
        Some("ducklake:lake/raw/catalog.ducklake")
    );
    assert_eq!(raw.data_path, "lake/raw/data/");
    assert_eq!(raw.clients, Clients::OneProcess);
    assert_eq!(raw.catalog.extension(), None);

    assert_eq!(
        gold.catalog.attach(None).as_deref(),
        Some("ducklake:sqlite:/srv/gold/catalog.sqlite")
    );
    assert_eq!(gold.data_path, "/srv/gold/data/");
    assert_eq!(gold.clients, Clients::LocalProcesses);
    // MG-REQ-0521.3: `sqlite` only for a `shared` catalog, `postgres` only for `cloud`.
    assert_eq!(gold.catalog.extension(), Some("sqlite"));

    assert_eq!(
        ops.catalog,
        Catalog::Postgres {
            secret: "secret:ops_catalog".to_owned()
        }
    );
    assert_eq!(ops.catalog.attach(None), None);
    assert_eq!(
        ops.catalog.attach(Some("dbname=ops host=pg")).as_deref(),
        Some("ducklake:postgres:dbname=ops host=pg")
    );
    assert_eq!(ops.data_path, "s3://ops");
    assert_eq!(ops.clients, Clients::Remote);
    assert_eq!(ops.catalog.extension(), Some("postgres"));

    // MG-REQ-0521.2: `catalog` and `data_path` override the defaults.
    let overridden = zones(
        "name = \"sales\"\n[gold]\nprofile = \"local\"\nlocation = \"gold\"\n\
         catalog = \"meta/gold.ducklake\"\ndata_path = \"/big/disk/\"\n",
    )
    .unwrap();
    assert_eq!(
        overridden[0].catalog.attach(None).as_deref(),
        Some("ducklake:meta/gold.ducklake")
    );
    assert_eq!(overridden[0].data_path, "/big/disk/");

    // Any other key is an error (MG-REQ-0361), and `cloud` cannot default its catalog.
    let other =
        zones("name = \"sales\"\n[gold]\nprofile = \"local\"\nlocation = \"g\"\nport = 1\n");
    assert_eq!(other.unwrap_err(), ["MG-0304 gold.port"]);
    let no_catalog =
        zones("name = \"sales\"\n[gold]\nprofile = \"cloud\"\nlocation = \"s3://g\"\n");
    assert_eq!(no_catalog.unwrap_err(), ["MG-0305 gold.catalog"]);
    let literal = zones(
        "name = \"sales\"\n[gold]\nprofile = \"cloud\"\nlocation = \"s3://g\"\ncatalog = \"pg\"\n",
    );
    assert_eq!(literal.unwrap_err(), ["MG-0305 gold.catalog"]);
    let no_location = zones("name = \"sales\"\n[gold]\nprofile = \"local\"\n");
    assert_eq!(no_location.unwrap_err(), ["MG-0305 gold.location"]);
    let server = zones("name = \"sales\"\n[gold]\nprofile = \"server\"\nlocation = \"g\"\n");
    assert_eq!(server.unwrap_err(), ["MG-0305 gold.profile"]);
}

#[test]
fn mg_req_0501_raw_and_gold_separate_credentials() {
    let problems = |text: &str| try_problems(text).unwrap();
    let both = |raw: &str, gold: &str| format!("name = \"sales\"\n[raw]\n{raw}\n[gold]\n{gold}\n");
    // MG-REQ-0501.1: `local` and `shared` locations differ and do not nest.
    let same = problems(&both(
        "profile = \"local\"\nlocation = \"lake\"",
        "profile = \"shared\"\nlocation = \"./lake/\"",
    ));
    assert_eq!(same.len(), 1);
    assert_eq!(same[0].code(), "MG-0501");
    assert_eq!(same[0].path(), "project.toml");
    assert_eq!(same[0].line(), 7);
    let message = same[0].to_string();
    assert!(
        message.contains("raw") && message.contains("gold"),
        "{message}"
    );
    assert!(message.contains("`lake`"), "{message}");

    let nested = problems(&both(
        "profile = \"local\"\nlocation = \"lake/raw\"",
        "profile = \"local\"\nlocation = \"lake\"",
    ));
    assert_eq!(nested.len(), 1);
    assert!(nested[0].to_string().contains("`lake`"), "{}", nested[0]);
    let inside = problems(&both(
        "profile = \"shared\"\nlocation = \"/data\"",
        "profile = \"shared\"\nlocation = \"/data/gold\"",
    ));
    assert_eq!(inside.len(), 1);

    let apart = problems(&both(
        "profile = \"local\"\nlocation = \"lake/raw\"",
        "profile = \"local\"\nlocation = \"lake/rawgold\"",
    ));
    assert!(apart.is_empty(), "{apart:?}");

    // MG-REQ-0501.2: with `cloud`, catalogs and location prefixes differ.
    let catalog = problems(&both(
        "profile = \"cloud\"\nlocation = \"s3://raw\"\ncatalog = \"secret:lake\"",
        "profile = \"cloud\"\nlocation = \"s3://gold\"\ncatalog = \"secret:lake\"",
    ));
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].code(), "MG-0501");
    assert_eq!(catalog[0].line(), 9);
    assert!(
        catalog[0].to_string().contains("`secret:lake`"),
        "{}",
        catalog[0]
    );

    let prefix = problems(&both(
        "profile = \"cloud\"\nlocation = \"s3://lake/raw\"\ncatalog = \"secret:raw\"",
        "profile = \"cloud\"\nlocation = \"s3://lake\"\ncatalog = \"secret:gold\"",
    ));
    assert_eq!(prefix.len(), 1);
    assert!(
        prefix[0].to_string().contains("`s3://lake`"),
        "{}",
        prefix[0]
    );

    let separate = problems(&both(
        "profile = \"cloud\"\nlocation = \"s3://raw\"\ncatalog = \"secret:raw\"",
        "profile = \"cloud\"\nlocation = \"s3://gold\"\ncatalog = \"secret:gold\"",
    ));
    assert!(separate.is_empty(), "{separate:?}");

    // Only raw and gold are compared; ops may share either one.
    let ops = problems(
        "name = \"sales\"\n[raw]\nprofile = \"local\"\nlocation = \"raw\"\n\
         [ops]\nprofile = \"local\"\nlocation = \"raw\"\n",
    );
    assert!(ops.is_empty(), "{ops:?}");
}
