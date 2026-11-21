//! MG-REQ-0208: a document is serialized as canonical JSON (RFC 8785) before it is hashed or
//! signed.

use mergus_contracts::{Contract, ValidDocument, canonical_json};
use serde_json::{Value, json};

#[path = "fixtures/generated/sample_v1.rs"]
mod sample_v1;

use sample_v1::{Sample, SampleKind, SampleLocation};

#[cfg(test)]
fn sample(tags: Vec<String>) -> ValidDocument {
    mergus_contracts::validate(&Sample {
        contract: "sample".to_owned(),
        contract_version: 1,
        name: "orders".to_owned(),
        kind: SampleKind::Table,
        location: SampleLocation {
            path: "sales/orders".to_owned(),
            depth: Some(2),
        },
        ratio: Some(0.5),
        public: None,
        tags: Some(tags),
        owner: None,
    })
    .unwrap()
}

#[test]
fn mg_req_0208_hash_uses_canonical_json() {
    let document = sample(vec!["daily".to_owned()]);
    let expected = r#"{"contract":"sample","contract_version":1,"kind":"table","location":{"depth":2,"path":"sales/orders"},"name":"orders","ratio":0.5,"tags":["daily"]}"#;
    assert_eq!(document.hash_input(), expected.as_bytes());
    let reordered: Value = serde_json::from_str(
        r#"{ "tags": ["daily"], "ratio": 5e-1, "name": "orders",
             "location": { "path": "sales/orders", "depth": 2.0 },
             "kind": "table", "contract_version": 1, "contract": "sample" }"#,
    )
    .unwrap();
    assert_eq!(canonical_json(&reordered).as_bytes(), document.hash_input());
    assert_ne!(
        sample(vec!["weekly".to_owned()]).hash_input(),
        document.hash_input()
    );
}

#[test]
fn mg_req_0208_rfc_8785_section_3_2_2_example() {
    let input: Value = serde_json::from_str(
        r#"{
          "numbers": [333333333.33333329, 1E30, 4.50, 2e-3, 0.000000000000000000000000001],
          "string": "\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/",
          "literals": [null, true, false]
        }"#,
    )
    .unwrap();
    let expected = "{\"literals\":[null,true,false],\"numbers\":[333333333.3333333,1e+30,4.5,0.002,1e-27],\"string\":\"\u{20ac}$\\u000f\\nA'B\\\"\\\\\\\\\\\"/\"}";
    assert_eq!(canonical_json(&input), expected);
}

#[test]
fn mg_req_0208_rfc_8785_section_3_2_3_property_order() {
    let input: Value = serde_json::from_str(
        r#"{
          "\u20ac": "Euro Sign",
          "\r": "Carriage Return",
          "\ufb33": "Hebrew Letter Dalet With Dagesh",
          "1": "One",
          "\ud83d\ude00": "Emoji: Grinning Face",
          "\u0080": "Control",
          "\u00f6": "Latin Small Letter O With Diaeresis"
        }"#,
    )
    .unwrap();
    let expected = "{\"\\r\":\"Carriage Return\",\"1\":\"One\",\"\u{80}\":\"Control\",\
                    \"\u{f6}\":\"Latin Small Letter O With Diaeresis\",\"\u{20ac}\":\"Euro Sign\",\
                    \"\u{1f600}\":\"Emoji: Grinning Face\",\
                    \"\u{fb33}\":\"Hebrew Letter Dalet With Dagesh\"}";
    assert_eq!(canonical_json(&input), expected);
}

/// RFC 8785, Appendix B: each IEEE 754 double, by its bits, and its canonical text.
const NUMBERS: [(u64, &str); 24] = [
    (0x0000_0000_0000_0000, "0"),
    (0x8000_0000_0000_0000, "0"),
    (0x0000_0000_0000_0001, "5e-324"),
    (0x8000_0000_0000_0001, "-5e-324"),
    (0x7fef_ffff_ffff_ffff, "1.7976931348623157e+308"),
    (0xffef_ffff_ffff_ffff, "-1.7976931348623157e+308"),
    (0x4340_0000_0000_0000, "9007199254740992"),
    (0xc340_0000_0000_0000, "-9007199254740992"),
    (0x4430_0000_0000_0000, "295147905179352830000"),
    (0x44b5_2d02_c7e1_4af5, "9.999999999999997e+22"),
    (0x44b5_2d02_c7e1_4af6, "1e+23"),
    (0x44b5_2d02_c7e1_4af7, "1.0000000000000001e+23"),
    (0x444b_1ae4_d6e2_ef4e, "999999999999999700000"),
    (0x444b_1ae4_d6e2_ef4f, "999999999999999900000"),
    (0x444b_1ae4_d6e2_ef50, "1e+21"),
    (0x3eb0_c6f7_a0b5_ed8c, "9.999999999999997e-7"),
    (0x3eb0_c6f7_a0b5_ed8d, "0.000001"),
    (0x41b3_de43_5555_5553, "333333333.3333332"),
    (0x41b3_de43_5555_5554, "333333333.33333325"),
    (0x41b3_de43_5555_5555, "333333333.3333333"),
    (0x41b3_de43_5555_5556, "333333333.3333334"),
    (0x41b3_de43_5555_5557, "333333333.33333343"),
    (0xbecb_f647_612f_3696, "-0.0000033333333333333333"),
    (0x4314_3ff3_c1cb_0959, "1424953923781206.2"),
];

#[test]
fn mg_req_0208_rfc_8785_appendix_b_numbers() {
    for (bits, expected) in NUMBERS {
        let number = json!(f64::from_bits(bits));
        assert_eq!(canonical_json(&number), expected, "{bits:#018x}");
    }
}

#[test]
fn mg_req_0208_strings_and_integers() {
    let input = json!({ "a": "\u{8}\t\n\u{c}\r\u{1f}\u{7f}\u{2028}", "b": [9_007_199_254_740_993_u64, -1, {}] });
    assert_eq!(
        canonical_json(&input),
        "{\"a\":\"\\b\\t\\n\\f\\r\\u001f\u{7f}\u{2028}\",\"b\":[9007199254740992,-1,{}]}"
    );
}
