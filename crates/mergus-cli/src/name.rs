//! Every name the binary shows, derived from the one product name (MG-REQ-1325).
//!
//! Nothing else in this crate spells the product name: help, completions, messages and
//! environment variables take it from here.

use mergus_contracts::names::BINARY;

/// The product name, as the binary, help, completions and messages show it.
pub(crate) const PRODUCT: &str = BINARY;

/// The prefix of every environment variable Mergus reads: the product name in upper case,
/// then `_`.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the configuration of later issues uses it")
)]
pub(crate) fn env_prefix() -> String {
    format!("{}_", PRODUCT.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_is_the_contract_binary_name() {
        assert_eq!(PRODUCT, mergus_contracts::names::BINARY);
    }

    #[test]
    fn env_prefix_is_the_upper_case_name_and_an_underscore() {
        let prefix = env_prefix();
        assert_eq!(
            prefix.strip_suffix('_'),
            Some(BINARY.to_ascii_uppercase().as_str())
        );
    }

    #[test]
    fn binary_target_is_named_after_the_product() {
        assert!(env!("CARGO_BIN_NAME").eq(PRODUCT));
    }
}
