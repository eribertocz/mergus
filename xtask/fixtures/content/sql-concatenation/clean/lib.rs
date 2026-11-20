fn f(conn: &C, table: &str, id: u32) {
    // sql: identifiers quoted by quote_identifier
    let a = format!("SELECT * FROM {}", quote_identifier(table));
    let b = "SELECT * FROM t WHERE id = ?";
    let c = format!("SELECT '{{}}' FROM t");
    let d = format!("{id} rows");
    let e = conn.query("SELECT {id}");
    let f = other!("DROP {}", table);
    // let g = format!("SELECT {id}");
    /* format!("SELECT {id}") */
}
