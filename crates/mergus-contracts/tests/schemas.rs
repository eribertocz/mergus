//! MG-REQ-0201: every contract has a JSON Schema 2020-12 with closed objects, `contract` and
//! `contract_version`, and at least one valid and one invalid example.

#[cfg(test)]
mod common;

#[test]
fn mg_req_0201_every_contract_has_schema_and_examples() {
    let schemas = common::workspace_root().join("schemas");
    assert_eq!(common::problems(&schemas), Vec::<String>::new());
    let conforming = common::fixtures().join("schemas");
    assert_eq!(common::problems(&conforming), Vec::<String>::new());
}

#[test]
fn mg_req_0201_nonconforming_contracts_are_reported() {
    let nonconforming = common::fixtures().join("nonconforming");
    assert_eq!(
        common::problems(&nonconforming),
        [
            "failing_valid/v1: valid example `example.json` is not valid",
            "no_examples/v1: no valid example",
            "no_examples/v1: no invalid example",
            "no_version/v1: `contract_version` is not a required property",
            "open_object/v1: the object at `/properties/extra` is not closed",
            "optional_contract/v1: `contract` is not a required property",
            "passing_invalid/v1: invalid example `example.json` is not invalid",
            "wrong_draft/v1: `$schema` is not `https://json-schema.org/draft/2020-12/schema`",
        ]
    );
}
