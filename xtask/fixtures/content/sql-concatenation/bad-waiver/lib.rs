fn f(table: &str) {
    // sql: identifiers quoted by
    let a = format!("SELECT * FROM {}", table);
    // sql: identifiers quoted by quote_identifier

    let b = format!(
        "SELECT {}",
        quote_identifier(table),
    );
}
