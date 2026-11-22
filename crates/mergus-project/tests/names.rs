//! Tests of the name rules.

use mergus_project::{closest_name, is_valid_name, suggest_valid_name};

#[test]
fn mg_req_0359_closest_name_by_edit_distance() {
    let candidates = ["customers", "orders", "order_lines"];
    assert_eq!(closest_name("custmers", candidates), Some("customers"));
    assert_eq!(closest_name("order", candidates), Some("orders"));
    // Ties are broken by byte order.
    assert_eq!(closest_name("ab", ["ac", "aa", "ad"]), Some("aa"));
    // At most `max(2, n / 3)`: 3 for 9 scalar values, 2 for 3.
    assert_eq!(closest_name("abcdefghi", ["abcdefxyz"]), Some("abcdefxyz"));
    assert_eq!(closest_name("abcdefghi", ["abcdewxyz"]), None);
    assert_eq!(closest_name("abc", ["xyc"]), Some("xyc"));
    assert_eq!(closest_name("abc", ["xyz"]), None);
    // Counted in Unicode scalar values, not bytes.
    assert_eq!(closest_name("xéé", ["x"]), Some("x"));
    assert_eq!(closest_name("ab", std::iter::empty()), None);
}

#[test]
fn mg_req_0360_invalid_name_gets_valid_suggestion() {
    let cases = [
        ("Customers", "customers"),
        ("my-model.v2", "my_model_v2"),
        ("a--b", "a_b"),
        ("a__-b", "a___b"),
        ("__x__", "x"),
        ("2024_sales", "n_2024_sales"),
        ("---", "n_"),
        ("", "n_"),
        ("Café", "caf"),
        ("ÉCOLE", "cole"),
        ("Country Codes", "country_codes"),
    ];
    for (given, expected) in cases {
        assert_eq!(
            suggest_valid_name(given),
            expected,
            "suggestion for {given:?}"
        );
        assert!(is_valid_name(expected), "{expected:?} is valid");
    }
    let long = "A".repeat(70);
    assert_eq!(suggest_valid_name(&long), "a".repeat(63));
}

#[test]
fn mg_req_0305_names_match_the_pattern() {
    for valid in ["a", "customers", "stg_orders_2", &"a".repeat(63)] {
        assert!(is_valid_name(valid), "{valid:?} is valid");
    }
    for invalid in [
        "",
        "Customers",
        "_a",
        "2a",
        "a-b",
        "a.b",
        "café",
        &"a".repeat(64),
    ] {
        assert!(!is_valid_name(invalid), "{invalid:?} is invalid");
    }
}
