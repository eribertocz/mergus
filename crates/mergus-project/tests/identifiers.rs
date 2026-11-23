//! Tests of stable identifiers and `ids.toml` (MG-REQ-0209, MG-REQ-0368 to MG-REQ-0371).

use std::path::Path;

use mergus_ports::{FileSystem, PortError};
use mergus_project::{
    IdKind, Identifier, Identifiers, LayoutError, Renames, derive_identifier, read_layout,
    read_renames,
};
use mergus_sim::SimFileSystem;

const ROOT: &str = "/project";

/// The derived identifiers of MG-REQ-0369, computed outside Mergus with Python's `hashlib`
/// and `base64.b32encode`.
const FACT_SALES: &str = "mdl_sw4a23b2ujmhopsw";
const FACT_SALES_1: &str = "mdl_7xxipd5swbdkv2gz";
const DIM_CUSTOMER: &str = "mdl_d7kqlbnlx3fyjv6l";
const ERP: &str = "src_pjwh5zisyhqmfjuq";
const ERP_ORDERS: &str = "stb_uvqsge53kt6pqv4s";
const ERP_ORDERS_AMOUNT: &str = "col_fnaymokm4mwblmaz";
const FACT_SALES_AMOUNT: &str = "col_2tx5cwlgfpqk77cy";

/// Writes `files`, given relative to the root, into a simulated file system.
fn project(files: &[(&str, &str)]) -> Result<SimFileSystem, PortError> {
    let fs = SimFileSystem::new();
    fs.create_dir_all(Path::new(ROOT))?;
    for (path, contents) in files {
        let path = Path::new(ROOT).join(path);
        if let Some(parent) = path.parent() {
            fs.create_dir_all(parent)?;
        }
        fs.write(&path, contents.as_bytes())?;
    }
    Ok(fs)
}

/// Returns the identifiers of the models of a simulated project, by model name.
fn model_ids(files: &[(&str, &str)]) -> Result<Vec<(String, String)>, String> {
    let fs = project(files).map_err(|error| error.to_string())?;
    let layout = read_layout(&fs, Path::new(ROOT)).map_err(|error| error.to_string())?;
    let renames = read_renames(&fs, Path::new(ROOT)).map_err(|error| error.to_string())?;
    let mut ids = Identifiers::new(&renames);
    let found = layout
        .models
        .iter()
        .map(|model| {
            let id = ids.model(&model.name, &model.path);
            (model.name.clone(), id.to_string())
        })
        .collect();
    ids.finish().map_err(|problems| format!("{problems:?}"))?;
    Ok(found)
}

/// Returns the problems of an `ids.toml` holding `text`.
macro_rules! problems {
    ($text:expr) => {
        match Renames::parse("ids.toml", $text) {
            Err(problems) => problems,
            Ok(renames) => panic!("expected problems, got {renames:?}"),
        }
    };
}

/// Returns the identifier written `text`.
macro_rules! id {
    ($text:expr) => {
        Identifier::parse($text).unwrap()
    };
}

#[test]
fn mg_req_0209_identifiers_survive_renames() {
    let before = model_ids(&[("models/staging/fact_sales.sql", "")]).unwrap();
    let moved = model_ids(&[("models/marts/deep/fact_sales.sql", "")]).unwrap();
    assert_eq!(before, moved);
    assert_eq!(before[0].1, FACT_SALES);

    let ids_toml = format!("[model]\nfact_orders = \"{FACT_SALES}\"\n");
    let renamed = model_ids(&[
        ("models/staging/fact_orders.sql", ""),
        ("ids.toml", &ids_toml),
    ])
    .unwrap();
    assert_eq!(
        renamed,
        vec![("fact_orders".to_owned(), FACT_SALES.to_owned())]
    );

    // A column keeps its identifier when its table is renamed.
    let renames = Renames::parse("ids.toml", &ids_toml).unwrap();
    let mut ids = Identifiers::new(&renames);
    let model = ids.model("fact_orders", "models/staging/fact_orders.sql");
    let column = ids.column(&model, "amount", "models/staging/fact_orders.sql");
    assert_eq!(column.as_str(), FACT_SALES_AMOUNT);
}

#[test]
fn mg_req_0368_identifier_format() {
    let renames = Renames::default();
    let mut ids = Identifiers::new(&renames);
    let model = ids.model("fact_sales", "models/fact_sales.sql");
    let source = ids.source("erp", "sources/erp.toml");
    let table = ids.source_table(&source, "orders", "sources/erp.toml");
    let column = ids.column(&table, "amount", "sources/erp.toml");
    for (found, kind, prefix) in [
        (&model, IdKind::Model, "mdl_"),
        (&source, IdKind::Source, "src_"),
        (&table, IdKind::SourceTable, "stb_"),
        (&column, IdKind::Column, "col_"),
    ] {
        let text = found.as_str();
        let body = text.strip_prefix(prefix).unwrap();
        assert_eq!(body.len(), 16, "{text}");
        assert!(
            body.bytes()
                .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
        );
        assert_eq!(found.kind(), kind);
        assert_eq!(Identifier::parse(text).as_ref(), Some(found));
    }
    for invalid in [
        "",
        "mdl_",
        "mdl_sw4a23b2ujmhops",
        "mdl_sw4a23b2ujmhopsww",
        "MDL_sw4a23b2ujmhopsw",
        "mdl_SW4A23B2UJMHOPSW",
        "mdl_sw4a23b2ujmhops0",
        "mdl_sw4a23b2ujmhops1",
        "mdl_sw4a23b2ujmhops8",
        "mdl_sw4a23b2ujmhops=",
        "mdl-sw4a23b2ujmhopsw",
        "tbl_sw4a23b2ujmhopsw",
        "sw4a23b2ujmhopsw",
    ] {
        assert_eq!(Identifier::parse(invalid), None, "{invalid}");
    }
}

#[test]
fn mg_req_0369_identifier_derived_from_name() {
    let renames = Renames::default();
    let mut ids = Identifiers::new(&renames);
    let model = ids.model("fact_sales", "models/fact_sales.sql");
    let source = ids.source("erp", "sources/erp.toml");
    let table = ids.source_table(&source, "orders", "sources/erp.toml");
    let column = ids.column(&table, "amount", "sources/erp.toml");
    assert_eq!(model.as_str(), FACT_SALES);
    assert_eq!(source.as_str(), ERP);
    assert_eq!(table.as_str(), ERP_ORDERS);
    assert_eq!(column.as_str(), ERP_ORDERS_AMOUNT);
    ids.finish().unwrap();

    assert_eq!(
        derive_identifier(IdKind::Model, None, "fact_sales", 0).as_str(),
        FACT_SALES
    );
    assert_eq!(
        derive_identifier(IdKind::Model, None, "fact_sales", 1).as_str(),
        FACT_SALES_1
    );
    assert_eq!(
        derive_identifier(IdKind::Column, Some(&id!(FACT_SALES)), "amount", 0).as_str(),
        FACT_SALES_AMOUNT
    );
}

/// Each `ids.toml` that MG-REQ-0370.1 rejects, with the line and the key of its problem.
fn invalid_ids_files() -> Vec<(String, usize, Option<&'static str>)> {
    vec![
        (format!("[table]\na = \"{FACT_SALES}\"\n"), 1, Some("table")),
        ("name = \"x\"\n".to_owned(), 1, Some("name")),
        ("[model]\na = 1\n".to_owned(), 2, Some("model.a")),
        ("[model]\na = \"mdl_x\"\n".to_owned(), 2, Some("model.a")),
        (format!("[model]\na = \"{ERP}\"\n"), 2, Some("model.a")),
        (
            format!("[source]\na = \"{FACT_SALES}\"\n"),
            2,
            Some("source.a"),
        ),
        ("[model.a]\nb = 1\n".to_owned(), 1, Some("model.a")),
        ("[model]\na = \n".to_owned(), 2, None),
    ]
}

#[test]
fn mg_req_0370_renamed_object_keeps_identifier() {
    // Item 1: only `[model]` and `[source]`, mapping names to identifiers of their prefix.
    let valid =
        format!("[model]\nfact_orders = \"{FACT_SALES}\"\n\n[source]\nerp_prod = \"{ERP}\"\n");
    let renames = Renames::parse("ids.toml", &valid).unwrap();
    assert_eq!(renames.model("fact_orders"), Some(&id!(FACT_SALES)));
    assert_eq!(renames.source("erp_prod"), Some(&id!(ERP)));
    assert!(Renames::parse("ids.toml", "").is_ok());
    for (text, line, key) in invalid_ids_files() {
        let found = problems!(&text);
        let first = &found[0];
        let first = (first.code(), first.path(), first.line(), first.key());
        assert_eq!(found.len(), 1, "{text}: {found:?}");
        assert_eq!(first, ("MG-0310", "ids.toml", Some(line), key), "{text}");
    }

    // Item 2: an object with an entry takes its identifier.
    let mut ids = Identifiers::new(&renames);
    assert_eq!(
        ids.model("fact_orders", "models/fact_orders.sql"),
        id!(FACT_SALES)
    );
    assert_eq!(ids.source("erp_prod", "sources/erp_prod.toml"), id!(ERP));
    ids.finish().unwrap();

    // Item 3: the same identifier twice names both keys.
    let twice = format!("[model]\nfact_orders = \"{FACT_SALES}\"\nfact_sales = \"{FACT_SALES}\"\n");
    let found = problems!(&twice);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].code(), "MG-0311");
    assert_eq!(found[0].line(), Some(3));
    let message = found[0].to_string();
    assert!(message.contains("`model.fact_orders`"), "{message}");
    assert!(message.contains("`model.fact_sales`"), "{message}");

    // Item 4: an entry that matches no object is kept and its identifier stays reserved.
    let unmatched = format!("[model]\nold_name = \"{FACT_SALES}\"\n");
    let renames = Renames::parse("ids.toml", &unmatched).unwrap();
    assert_eq!(renames.model("old_name"), Some(&id!(FACT_SALES)));
    let mut ids = Identifiers::new(&renames);
    assert_eq!(
        ids.model("fact_sales", "models/fact_sales.sql").as_str(),
        FACT_SALES_1
    );
    ids.finish().unwrap();
}

/// `ids.toml` is read at the project root; a missing file has no entry (MG-REQ-0370.1).
#[test]
fn mg_req_0370_ids_toml_read_at_project_root() {
    let fs = project(&[]).unwrap();
    assert_eq!(
        read_renames(&fs, Path::new(ROOT)).unwrap(),
        Renames::default()
    );
    let fs = project(&[("ids.toml", "[other]\n")]).unwrap();
    match read_renames(&fs, Path::new(ROOT)) {
        Err(LayoutError::Invalid(found)) => assert_eq!(found[0].code(), "MG-0310"),
        other => panic!("expected problems, got {other:?}"),
    }
}

#[test]
fn mg_req_0371_identifier_never_reused() {
    // An identifier stored in `ids.toml` is not derived for another object.
    let stored = format!("[model]\ndim_customer = \"{FACT_SALES}\"\n");
    let renames = Renames::parse("ids.toml", &stored).unwrap();
    let mut ids = Identifiers::new(&renames);
    let customer = ids.model("dim_customer", "models/dim_customer.sql");
    let sales = ids.model("fact_sales", "models/fact_sales.sql");
    assert_eq!(customer.as_str(), FACT_SALES);
    assert_eq!(sales.as_str(), FACT_SALES_1);
    assert_ne!(customer.as_str(), DIM_CUSTOMER);
    ids.finish().unwrap();

    // Two objects resolving to the same identifier are an error naming both.
    let renames = Renames::default();
    let mut ids = Identifiers::new(&renames);
    let first = ids.model("fact_sales", "models/staging/fact_sales.sql");
    let second = ids.model("fact_sales", "models/marts/fact_sales.sql");
    assert_eq!(first, second);
    let found = ids.finish().unwrap_err();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].code(), "MG-0312");
    assert_eq!(found[0].path(), "models/marts/fact_sales.sql");
    assert_eq!(found[0].line(), None);
    assert_eq!(
        found[0].to_string(),
        format!(
            "model `fact_sales` (`models/staging/fact_sales.sql`) and model `fact_sales` \
             (`models/marts/fact_sales.sql`) have the same identifier `{FACT_SALES}`"
        )
    );

    // A column is named after its table.
    let mut ids = Identifiers::new(&renames);
    let source = ids.source("erp", "sources/erp.toml");
    let table = ids.source_table(&source, "orders", "sources/erp.toml");
    ids.column(&table, "amount", "sources/erp.toml");
    ids.column(&table, "amount", "sources/erp.toml");
    let found = ids.finish().unwrap_err();
    assert!(
        found[0]
            .to_string()
            .starts_with("column `amount` of table `orders` of source `erp` (`sources/erp.toml`)"),
        "{found:?}"
    );
}
