//! Tests of reading the project layout through the simulated file system.

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_project::{
    Layout, LayoutError, Model, ModelLanguage, NameKind, ProjectError, Publication, read_layout,
};
use mergus_sim::SimFileSystem;

const ROOT: &str = "/project";

/// Reads the layout of a simulated project holding `files`, given relative to the root with
/// their contents.
fn read(files: &[(&str, &str)]) -> Result<Layout, LayoutError> {
    let fs = SimFileSystem::new();
    for (path, contents) in files {
        let path = Path::new(ROOT).join(path);
        if let Some(parent) = path.parent() {
            fs.create_dir_all(parent)?;
        }
        fs.write(&path, contents.as_bytes())?;
    }
    read_layout(&fs, Path::new(ROOT))
}

/// Returns the problems of a project that has some.
macro_rules! problems {
    ($files:expr) => {
        match read($files) {
            Err(LayoutError::Invalid(problems)) => problems,
            other => panic!("expected problems, got {other:?}"),
        }
    };
}

fn model(name: &str, path: &str, language: ModelLanguage, publication: Publication) -> Model {
    Model {
        name: name.to_owned(),
        path: path.to_owned(),
        language,
        publication,
    }
}

#[test]
fn mg_req_0301_comments_have_no_meaning() {
    let names = [
        "models/gold/sales/orders.sql",
        "models/staging/stg_orders.sql",
        "models/staging/clean.py",
    ];
    let plain: Vec<_> = names.iter().map(|name| (*name, "select 1")).collect();
    let annotated: Vec<_> = names
        .iter()
        .map(|name| {
            (
                *name,
                "-- mergus: publish = false\n/* materialized = 'view' */\n\
                 # name = other\n@model(publish=False)\nimport mergus\nselect 1",
            )
        })
        .collect();
    let plain = read(&plain).unwrap();
    assert_eq!(read(&annotated).unwrap(), plain);
    assert_eq!(
        plain.models[0].published_name().as_deref(),
        Some("gold.sales.orders")
    );
}

#[test]
fn mg_req_0302_one_file_defines_one_model() {
    let layout = read(&[
        ("models/staging/stg_orders.sql", "select 1"),
        ("models/staging/mergus.toml", ""),
        ("models/clean.py", "x = 1"),
        ("models/README.md", "notes"),
        ("project.toml", ""),
    ])
    .unwrap();
    assert_eq!(
        layout.models,
        vec![
            model(
                "clean",
                "models/clean.py",
                ModelLanguage::Python,
                Publication::Intermediate
            ),
            model(
                "stg_orders",
                "models/staging/stg_orders.sql",
                ModelLanguage::Sql,
                Publication::Intermediate
            ),
        ]
    );
}

#[test]
fn mg_req_0303_duplicate_model_names_rejected() {
    let problems = problems!(&[
        ("models/a/customers.sql", ""),
        ("models/b/customers.py", ""),
        ("models/c/orders.sql", ""),
    ]);
    assert_eq!(
        problems,
        vec![ProjectError::DuplicateModel {
            name: "customers".to_owned(),
            first: "models/a/customers.sql".to_owned(),
            second: "models/b/customers.py".to_owned(),
        }]
    );
    assert_eq!(problems[0].code(), "MG-0301");
    assert_eq!(problems[0].path(), "models/b/customers.py");
    let message = problems[0].to_string();
    assert!(message.contains("models/a/customers.sql"), "{message}");
    assert!(message.contains("models/b/customers.py"), "{message}");
}

#[test]
fn mg_req_0304_gold_path_defines_published_name() {
    let layout = read(&[
        ("models/gold/sales/orders.sql", ""),
        ("models/gold/sales/deep/lines.sql", ""),
        ("models/gold/top.sql", ""),
        ("models/silver/sales/returns.py", ""),
    ])
    .unwrap();
    let published: Vec<_> = layout
        .models
        .iter()
        .map(|model| (model.name.as_str(), model.published_name()))
        .collect();
    assert_eq!(
        published,
        vec![
            ("lines", None),
            ("orders", Some("gold.sales.orders".to_owned())),
            ("top", None),
            ("returns", None),
        ]
    );
    assert_eq!(
        layout.models[1].publication,
        Publication::Gold {
            schema: "sales".to_owned(),
            table: "orders".to_owned()
        }
    );
}

fn invalid(kind: NameKind, name: &str, path: &str, suggestion: &str) -> ProjectError {
    ProjectError::InvalidName {
        kind,
        name: name.to_owned(),
        path: path.to_owned(),
        suggestion: suggestion.to_owned(),
    }
}

/// A project with an invalid model, folder, schema, source and seed name.
const INVALID_NAMES: &[(&str, &str)] = &[
    ("models/Customers.sql", ""),
    ("models/My Folder/ok.sql", ""),
    ("models/gold/Sales-EU/orders.sql", ""),
    ("models/gold/sales/orders.sql", ""),
    ("sources/CRM.toml", ""),
    ("sources/erp.toml", ""),
    ("seeds/Country Codes.csv", ""),
    ("seeds/currencies.csv", ""),
];

/// The problems of [`INVALID_NAMES`], in walking order.
fn invalid_names_problems() -> Vec<ProjectError> {
    vec![
        invalid(
            NameKind::Model,
            "Customers",
            "models/Customers.sql",
            "customers",
        ),
        invalid(
            NameKind::Folder,
            "My Folder",
            "models/My Folder",
            "my_folder",
        ),
        invalid(
            NameKind::Schema,
            "Sales-EU",
            "models/gold/Sales-EU",
            "sales_eu",
        ),
        invalid(NameKind::Source, "CRM", "sources/CRM.toml", "crm"),
        invalid(
            NameKind::Seed,
            "Country Codes",
            "seeds/Country Codes.csv",
            "country_codes",
        ),
        ProjectError::DuplicateModel {
            name: "orders".to_owned(),
            first: "models/gold/Sales-EU/orders.sql".to_owned(),
            second: "models/gold/sales/orders.sql".to_owned(),
        },
    ]
}

#[test]
fn mg_req_0305_invalid_names_rejected_with_suggestion() {
    let problems = problems!(INVALID_NAMES);
    assert_eq!(problems, invalid_names_problems());
    assert!(
        problems[..5]
            .iter()
            .all(|problem| problem.code() == "MG-0302")
    );
    assert_eq!(problems[2].path(), "models/gold/Sales-EU");
    assert!(problems[3].to_string().contains("`crm`"));
}

#[test]
fn missing_folders_give_an_empty_layout() {
    assert_eq!(read(&[("project.toml", "")]).unwrap(), Layout::default());
}
