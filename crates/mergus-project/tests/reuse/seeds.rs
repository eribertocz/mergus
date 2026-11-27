//! Seeds: declared, typed, hashed, small.

use std::path::Path;

use mergus_contracts::hash::sha256_hex;
use mergus_ports::ResultColumn;
use mergus_project::{
    LARGE_SEED_BYTES, LayoutError, ProjectError, ProjectWarning, Seeds, read_configuration,
    read_seeds,
};

use super::{ROOT, project};

/// The `project.toml` of these tests.
const PROJECT: &str = "name = \"sales\"\n\
\n\
[seeds.cities]\n\
columns = { code = \"VARCHAR\" }\n\
\n\
[seeds.countries]\n\
columns = { code = \"VARCHAR\", population = \"BIGINT\" }\n\
delimiter = \";\"\n\
\n\
[seeds.rates]\n\
columns = { day = \"DATE\", rate = \"decimal(18,3)\" }\n";

/// Reads the seeds of a project holding `project.toml` and `files`.
fn read(project_toml: &str, files: &[(&str, &[u8])]) -> Result<Seeds, LayoutError> {
    let mut all: Vec<(&str, &[u8])> = vec![("project.toml", project_toml.as_bytes())];
    all.extend_from_slice(files);
    let fs = project(&all)?;
    let configuration = read_configuration(&fs, Path::new(ROOT))?;
    read_seeds(&fs, Path::new(ROOT), &configuration)
}

#[test]
fn mg_req_0645_seed_declared_typed_and_hashed() {
    let countries: &[u8] = b"code;population\nPE;34000000\n";
    let files = [
        ("seeds/countries.csv", countries),
        ("seeds/rates.csv", b"day,rate\n".as_slice()),
        ("seeds/regions.csv", b"code\n".as_slice()),
    ];
    let read = read(PROJECT, &files).unwrap();
    let names: Vec<&str> = read.seeds.iter().map(|seed| seed.name.as_str()).collect();
    assert_eq!(names, ["countries"]);
    let seed = &read.seeds[0];
    assert_eq!(seed.path, "seeds/countries.csv");
    assert_eq!(seed.table(), "raw.seed__countries");
    assert_eq!(seed.delimiter, ";");
    assert!(seed.header);
    let column = |name: &str, data_type: &str| ResultColumn {
        name: name.to_owned(),
        data_type: data_type.to_owned(),
    };
    assert_eq!(
        seed.columns,
        [column("code", "VARCHAR"), column("population", "BIGINT")]
    );
    assert_eq!(seed.hash, format!("sha256:{}", sha256_hex(countries)));
    let references = ["raw.seed__countries".to_owned(), "stg_orders".to_owned()];
    let inputs = read.fingerprint_inputs(&references);
    let entries: Vec<(&str, &str)> = inputs
        .iter()
        .map(|(table, hash)| (table.as_str(), hash.as_str()))
        .collect();
    assert_eq!(entries, [("raw.seed__countries", seed.hash.as_str())]);
    assert!(
        read.fingerprint_inputs(&["stg_orders".to_owned()])
            .is_empty()
    );
    let changed = [(
        "seeds/countries.csv",
        b"code;population\nPE;34000001\n".as_slice(),
    )];
    let changed = self::read(PROJECT, &changed);
    assert_ne!(changed.unwrap().seeds[0].hash, seed.hash);

    assert_declaration_problems(&read);
    assert!(read.warnings.is_empty());
}

/// Checks the problems of the declarations of [`PROJECT`] (MG-REQ-0645.2): an unknown type,
/// an undeclared CSV and a declaration without CSV.
fn assert_declaration_problems(read: &Seeds) {
    let shown: Vec<(&str, Option<usize>, String)> = read
        .problems
        .iter()
        .map(|problem| (problem.code(), problem.line(), problem.to_string()))
        .collect();
    assert_eq!(shown.len(), 3, "{shown:#?}");
    assert_eq!((shown[0].0, shown[0].1), ("MG-0609", Some(11)));
    assert!(shown[0].2.contains("`decimal(18,3)`"), "{}", shown[0].2);
    assert_eq!(
        read.problems[1],
        ProjectError::UndeclaredSeed {
            name: "regions".to_owned(),
            path: "seeds/regions.csv".to_owned(),
        }
    );
    assert_eq!(shown[1].0, "MG-0606");
    assert_eq!(
        read.problems[2],
        ProjectError::SeedWithoutFile {
            name: "cities".to_owned(),
            path: "project.toml".to_owned(),
            line: 3,
        }
    );
    assert_eq!(shown[2].0, "MG-0607");
}

#[test]
fn mg_req_0645_unknown_seed_key_rejected() {
    let project =
        "name = \"sales\"\n[seeds.countries]\ncolumns = { code = \"VARCHAR\" }\nquote = \"'\"\n";
    let Err(LayoutError::Invalid(problems)) = read(project, &[]) else {
        panic!("expected problems");
    };
    assert_eq!(
        problems,
        [ProjectError::UnknownSeedKey {
            path: "project.toml".to_owned(),
            line: 4,
            key: "seeds.countries.quote".to_owned(),
            suggestion: None,
        }]
    );
    assert_eq!(problems[0].code(), "MG-0608");
    let missing = "name = \"sales\"\n[seeds.countries]\nheader = false\n";
    let Err(LayoutError::Invalid(problems)) = read(missing, &[]) else {
        panic!("expected problems");
    };
    assert_eq!(problems[0].code(), "MG-0305");
}

#[test]
fn mg_req_0646_large_seed_warns() {
    let project = "name = \"sales\"\n\
        [seeds.big]\ncolumns = { a = \"VARCHAR\" }\n\
        [seeds.edge]\ncolumns = { a = \"VARCHAR\" }\n";
    let limit = usize::try_from(LARGE_SEED_BYTES).unwrap();
    let big = vec![b'a'; limit + 1];
    let edge = vec![b'a'; limit];
    let files = [
        ("seeds/big.csv", big.as_slice()),
        ("seeds/edge.csv", edge.as_slice()),
    ];
    let read = read(project, &files).unwrap();
    assert!(read.problems.is_empty());
    assert_eq!(read.seeds.len(), 2, "a warning never fails the read");
    assert_eq!(
        read.warnings,
        [ProjectWarning::LargeSeed {
            path: "seeds/big.csv".to_owned(),
            bytes: 10_000_001,
        }]
    );
    assert_eq!(read.warnings[0].code(), "MG-0610");
    assert_eq!(read.warnings[0].path(), "seeds/big.csv");
    assert!(read.warnings[0].to_string().contains("source"));
}

#[test]
fn mg_req_0645_seed_columns_keep_declaration_order() {
    let project = "name = \"sales\"\n\
        [seeds.inline]\nheader = false\ncolumns = { zone = \"VARCHAR\", amount = \"BIGINT\" }\n\
        [seeds.table.columns]\nzz = \"DATE\"\nmm = \"VARCHAR\"\naa = \"BIGINT\"\n";
    let files = [
        ("seeds/inline.csv", b"north,7\n".as_slice()),
        ("seeds/table.csv", b"zz,mm,aa\n".as_slice()),
    ];
    let read = read(project, &files).unwrap();
    let names = |index: usize| -> Vec<&str> {
        read.seeds[index]
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect()
    };
    assert!(!read.seeds[0].header);
    assert_eq!(names(0), ["zone", "amount"]);
    assert_eq!(names(1), ["zz", "mm", "aa"]);
}
