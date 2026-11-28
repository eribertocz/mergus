//! Steps return recipes, never data (MG-REQ-0616).

use super::{problems, summary};

#[test]
fn mg_req_0616_collect_rejected() {
    let eager = |line: usize, column: usize, call: &str| {
        (
            "MG-0624",
            Some(line),
            format!(
                "`models/eager.py` line {line} column {column}: `{call}` makes the data eager; \
                 return the LazyFrame"
            ),
        )
    };
    // The private helper `_rows` counts too. `cs.DataFrame()` is not on the name `polars` is
    // imported as, and `collect(frame)` is a function, not a method.
    assert_eq!(
        summary(&problems("eager")),
        [
            eager(6, 12, ".to_dicts()"),
            eager(10, 12, ".collect()"),
            eager(14, 13, "pl.DataFrame()"),
            eager(17, 5, ".fetch()"),
            eager(22, 12, ".collect_async()"),
            eager(26, 5, ".to_pandas()"),
            eager(27, 12, ".to_arrow()"),
        ]
    );
}
