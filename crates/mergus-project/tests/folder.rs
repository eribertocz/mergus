//! Tests of folder `mergus.toml` files through the simulated file system.

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_project::{
    ConfigKind, Effective, LayoutError, ModelOptions, Origin, ProjectError, Publication,
    read_layout, read_model_options,
};
use mergus_sim::SimFileSystem;
use serde_json::{Value, json};

const ROOT: &str = "/project";

/// Reads the model options of a simulated project holding `files`, relative to the root.
fn read(files: &[(&str, &str)]) -> Result<Vec<ModelOptions>, LayoutError> {
    let fs = SimFileSystem::new();
    for (path, contents) in files {
        let path = Path::new(ROOT).join(path);
        if let Some(parent) = path.parent() {
            fs.create_dir_all(parent)?;
        }
        fs.write(&path, contents.as_bytes())?;
    }
    let layout = read_layout(&fs, Path::new(ROOT))?;
    read_model_options(&fs, Path::new(ROOT), &layout)
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

/// Returns the options of the model `name`.
macro_rules! model {
    ($models:expr, $name:expr) => {
        $models.iter().find(|model| model.name == $name).unwrap()
    };
}

/// Returns the single setting of `option`.
macro_rules! single {
    ($model:expr, $option:expr) => {
        match &$model.options[$option] {
            Effective::Single(setting) => setting,
            Effective::List(_) => panic!("{} is a list", $option),
        }
    };
}

/// Returns the items of the list option `option`.
macro_rules! list {
    ($model:expr, $option:expr) => {
        match &$model.options[$option] {
            Effective::List(items) => items.as_slice(),
            Effective::Single(_) => panic!("{} is not a list", $option),
        }
    };
}

fn file(path: &str, line: usize) -> Origin {
    Origin::File {
        path: path.to_owned(),
        line,
    }
}

const SALES: &str = "models/gold/sales/mergus.toml";

/// A project with three levels of `[all]` and two models in `models/gold/sales/`.
fn nested(sales: &str) -> Result<Vec<ModelOptions>, LayoutError> {
    read(&[
        (
            "models/mergus.toml",
            "[all]\nlookback = 1\nfresh = \"1d\"\nowner = \"root\"\n",
        ),
        (
            "models/gold/mergus.toml",
            "[all]\nlookback = 2\nfresh = \"2h\"\n",
        ),
        (SALES, sales),
        ("models/gold/sales/orders.sql", ""),
        ("models/gold/sales/items.sql", ""),
    ])
}

#[test]
fn mg_req_0331_section_for_missing_model_rejected() {
    let found = problems!(&[
        ("models/sales/fact_sales.sql", ""),
        (
            "models/sales/mergus.toml",
            "[all]\nlookback = 1\n\n[fact_sale]\nlookback = 2\n",
        ),
    ]);
    assert_eq!(
        found,
        [ProjectError::UnknownSection {
            path: "models/sales/mergus.toml".to_owned(),
            line: 4,
            section: "fact_sale".to_owned(),
            suggestion: Some("fact_sales".to_owned()),
        }]
    );
    assert_eq!(found[0].code(), "MG-0308");
    let other_folder = problems!(&[
        ("models/a/orders.sql", ""),
        ("models/b/mergus.toml", "[orders]\nlookback = 1\n"),
    ]);
    assert_eq!(other_folder[0].code(), "MG-0308");
    assert_eq!(other_folder[0].key(), Some("orders"));
}

#[test]
fn mg_req_0332_option_precedence() {
    let models = nested("[all]\nlookback = 3\n\n[orders]\nlookback = 4\n").unwrap();
    let orders = model!(&models, "orders");
    let items = model!(&models, "items");
    assert_eq!(single!(orders, "lookback").value, 4);
    assert_eq!(single!(items, "lookback").value, 3);
    assert_eq!(single!(items, "fresh").value, "2h");
    assert_eq!(single!(items, "owner").value, "root");
    assert_eq!(single!(items, "refresh").value, "full");
    assert_eq!(single!(items, "publish").value, "gold.sales.items");
}

#[test]
fn mg_req_0333_list_options_accumulate() {
    let models = read(&[
        (
            "models/mergus.toml",
            "[all]\nchecks = [\"c\"]\ntags = [\"t\"]\n",
        ),
        (
            "models/sales/mergus.toml",
            "[all]\nchecks = [\"b\", \"a\"]\nnondeterministic = [\"x\"]\n\
             [orders]\nchecks = [\"a\"]\nnondeterministic = [\"y\"]\n",
        ),
        ("models/sales/orders.sql", ""),
    ])
    .unwrap();
    let orders = model!(&models, "orders");
    let checks: Vec<&Value> = list!(orders, "checks")
        .iter()
        .map(|item| &item.value)
        .collect();
    assert_eq!(checks, [&json!("a"), &json!("b"), &json!("c")]);
    let tags: Vec<&Value> = list!(orders, "tags")
        .iter()
        .map(|item| &item.value)
        .collect();
    assert_eq!(tags, [&json!("t")]);
    assert_eq!(single!(orders, "nondeterministic").value, json!(["y"]));
}

#[test]
fn mg_req_0334_inherit_false_skips_parents() {
    let models = nested("[all]\nchecks = [\"own\"]\n\n[orders]\ninherit = false\n").unwrap();
    let orders = model!(&models, "orders");
    assert_eq!(single!(orders, "lookback").value, 0);
    assert_eq!(single!(orders, "fresh").value, Value::Null);
    assert_eq!(list!(orders, "checks")[0].value, "own");
    assert_eq!(single!(orders, "inherit").value, false);
    let items = model!(&models, "items");
    assert_eq!(single!(items, "lookback").value, 2);
    assert_eq!(single!(items, "inherit").value, true);
}

#[test]
fn mg_req_0335_origin_recorded_for_each_value() {
    let models = nested("[orders]\nrefresh = \"incremental\"\n").unwrap();
    let orders = model!(&models, "orders");
    let keys: Vec<&String> = orders.options.keys().collect();
    assert_eq!(
        keys,
        [
            "checks",
            "fresh",
            "inherit",
            "key",
            "lookback",
            "materialize",
            "nondeterministic",
            "output",
            "owner",
            "partition_by",
            "publish",
            "refresh",
            "tags",
        ]
    );
    assert_eq!(single!(orders, "refresh").origin, file(SALES, 2));
    assert_eq!(
        single!(orders, "lookback").origin,
        file("models/gold/mergus.toml", 2)
    );
    assert_eq!(
        single!(orders, "owner").origin,
        file("models/mergus.toml", 4)
    );
    assert_eq!(single!(orders, "materialize").origin, Origin::Default);
}

#[test]
fn mg_req_0362_only_due_keys_are_known() {
    let schema: Value = serde_json::from_str(ConfigKind::Folder.schema()).unwrap();
    let top: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
    assert_eq!(top, ["all"]);
    let options = [
        "checks",
        "fresh",
        "key",
        "lookback",
        "materialize",
        "nondeterministic",
        "output",
        "owner",
        "partition_by",
        "publish",
        "refresh",
        "tags",
    ];
    let all: Vec<&String> = schema["$defs"]["all"]["properties"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    assert_eq!(all, options);
    let model: Vec<&String> = schema["$defs"]["model"]["properties"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    let mut with_inherit = options.to_vec();
    with_inherit.insert(2, "inherit");
    assert_eq!(model, with_inherit);
    for later in [
        "pii = {}",
        "by = [\"a\"]",
        "deletes = \"soft\"",
        "inputs = {}",
    ] {
        let text = format!("[orders]\n{later}\n");
        let found = problems!(&[("models/orders.sql", ""), ("models/mergus.toml", &text)]);
        assert_eq!(found[0].code(), "MG-0304", "{later}");
    }
}

#[test]
fn mg_req_0366_option_values_validated() {
    let valid = "[all]\npublish = false\nrefresh = \"history\"\nkey = [\"id\", \"day\"]\n\
                 partition_by = \"day:quarter\"\nlookback = 0\nfresh = \"30m\"\n\
                 checks = [\"not_null(*_id)\"]\nnondeterministic = [\"random()\"]\n\
                 tags = [\"daily\"]\nowner = \"team\"\nmaterialize = true\noutput = \"locked\"\n\
                 [orders]\npublish = \"gold.a.b\"\nkey = \"id\"\ninherit = false\n";
    assert!(read(&[("models/orders.sql", ""), ("models/mergus.toml", valid)]).is_ok());
    let invalid = [
        "publish = \"silver.a.b\"",
        "publish = true",
        "refresh = \"partial\"",
        "key = []",
        "key = \"Id\"",
        "partition_by = \"day:hour\"",
        "lookback = -1",
        "fresh = \"0h\"",
        "fresh = \"6s\"",
        "checks = [\"\"]",
        "tags = [\"Daily\"]",
        "owner = \"\"",
        "materialize = \"yes\"",
        "output = \"open\"",
    ];
    for option in invalid {
        let text = format!("[orders]\n{option}\n");
        let found = problems!(&[("models/orders.sql", ""), ("models/mergus.toml", &text)]);
        assert_eq!(found.len(), 1, "{option}");
        assert_eq!(found[0].code(), "MG-0305", "{option}");
        assert_eq!(found[0].line(), Some(2), "{option}");
    }
    let inherit_in_all = problems!(&[
        ("models/orders.sql", ""),
        ("models/mergus.toml", "[all]\ninherit = false\n"),
    ]);
    assert_eq!(inherit_in_all[0].code(), "MG-0304");
    assert_eq!(inherit_in_all[0].key(), Some("all.inherit"));
}

#[test]
fn mg_req_0367_origin_shape() {
    let models =
        nested("[all]\nchecks = [\"x\"]\n[orders]\npublish = false\nchecks = [\"x\", \"y\"]\n")
            .unwrap();
    let orders = model!(&models, "orders");
    assert_eq!(single!(orders, "publish").value, false);
    assert_eq!(single!(orders, "publish").origin, file(SALES, 4));
    assert_eq!(orders.publication, Publication::Intermediate);
    let origins: Vec<&Origin> = list!(orders, "checks")
        .iter()
        .map(|item| &item.origin)
        .collect();
    assert_eq!(origins, [&file(SALES, 5), &file(SALES, 5)]);
    let items = model!(&models, "items");
    assert_eq!(single!(items, "publish").origin, Origin::Convention);
    assert_eq!(list!(items, "checks")[0].origin, file(SALES, 2));
    assert!(list!(items, "tags").is_empty());
    assert_eq!(single!(items, "refresh").origin, Origin::Default);

    let moved = read(&[
        ("models/staging/orders.sql", ""),
        (
            "models/staging/mergus.toml",
            "[orders]\npublish = \"gold.sales.orders\"\n",
        ),
        ("models/staging/lines.sql", ""),
    ])
    .unwrap();
    let orders = model!(&moved, "orders");
    assert_eq!(
        orders.publication,
        Publication::Gold {
            schema: "sales".to_owned(),
            table: "orders".to_owned(),
        }
    );
    assert_eq!(
        single!(orders, "publish").origin,
        file("models/staging/mergus.toml", 2)
    );
    let lines = model!(&moved, "lines");
    assert_eq!(single!(lines, "publish").value, false);
    assert_eq!(single!(lines, "publish").origin, Origin::Convention);
}
