//! MG-REQ-1305: selections and `mergus ls`.

use super::{mergus, shop};

/// The models `mergus ls <args>` selects, in the order it prints them.
fn listed(args: &[&str]) -> Vec<String> {
    let mut full = vec!["ls"];
    full.extend_from_slice(args);
    let outcome = mergus(&shop(), &full);
    assert_eq!(outcome.exit, 0, "{args:?}: {outcome:?}");
    outcome
        .stdout
        .lines()
        .map(|line| line.split(':').next().unwrap().to_owned())
        .collect()
}

#[test]
fn mg_req_1305_selectors() {
    let all = ["dim_customer", "fact_sales", "stg_customers", "stg_orders"];
    let cases: [(&[&str], &[&str]); 14] = [
        (&[], &all),
        (&["stg_orders"], &["stg_orders"]),
        (&["stg_orders+"], &["fact_sales", "stg_orders"]),
        (
            &["+fact_sales"],
            &["fact_sales", "stg_customers", "stg_orders"],
        ),
        (&["+dim_customer+"], &["dim_customer", "stg_customers"]),
        (&["tag:staging"], &["stg_customers", "stg_orders"]),
        (&["tag:finance"], &["fact_sales"]),
        (&["source:erp"], &["stg_orders"]),
        (&["path:models/gold"], &["dim_customer", "fact_sales"]),
        (&["path:models/gold/sales/fact_sales.sql"], &["fact_sales"]),
        (&["changed"], &all),
        (
            &["stg_orders", "dim_customer"],
            &["dim_customer", "stg_orders"],
        ),
        (&["tag:staging,stg_orders+"], &["stg_orders"]),
        (
            &["changed", "--exclude", "tag:staging"],
            &["dim_customer", "fact_sales"],
        ),
    ];
    for (args, expected) in cases {
        assert_eq!(listed(args), expected, "{args:?}");
    }
    assert_eq!(
        listed(&["changed", "--exclude", "stg_orders+ dim_customer"]),
        ["stg_customers"]
    );
}

#[test]
fn mg_req_1305_ls_prints_why() {
    let outcome = mergus(&shop(), &["ls", "stg_orders+", "tag:finance"]);
    assert_eq!(
        outcome.stdout,
        "fact_sales: downstream of `stg_orders`; has tag `finance`\nstg_orders: named\n"
    );
    let json = mergus(&shop(), &["ls", "source:crm", "--json"]);
    assert_eq!(json.exit, 0);
    assert!(json.of("run_finished").len() == 1);
    assert_eq!(json.stderr, "stg_customers: reads source `crm`\n");
}

#[test]
fn mg_req_1305_invalid_selectors() {
    let invalid = ["tag:", "owner:ana", "++stg_orders", "+", "stg+orders"];
    for word in invalid {
        let outcome = mergus(&shop(), &["ls", word, "--json"]);
        assert_eq!(outcome.exit, 1, "{word}");
        assert_eq!(outcome.of("diagnostic")[0]["code"], "MG-1309", "{word}");
    }
    let unknown = mergus(&shop(), &["ls", "+stg_order", "--json"]);
    assert_eq!(unknown.exit, 1);
    let diagnostic = &unknown.of("diagnostic")[0];
    assert_eq!(diagnostic["code"], "MG-1310");
    assert_eq!(diagnostic["hint"], "Use `stg_orders`.");
    let source = mergus(&shop(), &["ls", "source:erq", "--json"]);
    assert_eq!(source.of("diagnostic")[0]["hint"], "Use `erp`.");
}
