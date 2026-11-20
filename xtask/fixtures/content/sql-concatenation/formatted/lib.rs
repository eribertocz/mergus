fn f(out: &mut String, table: &str, id: u32) -> R {
    let _ = 1;
    let a = format!("SELECT * FROM t WHERE id = {id}");
    let b = format!("  select {} from t", id);
    write!(out, "INSERT INTO t VALUES ({})", id)?;
    writeln!(out, "update t set a = {id}")?;
    let c = format!("{} SELECT", id);
    let d = std::format!("DELETE FROM t WHERE id = {id}");
    let e = format!("MERGE INTO t USING {table}");
    let g = format!("CREATE TABLE {table} (a INT)");
    let h = format!("drop table {table}");
    let i = format!("ALTER TABLE {table} ADD b INT");
    let j = format!("WITH x AS (SELECT {id}) SELECT * FROM x");
    let k = format!(r#"COPY t TO '{table}'"#);
    let l = format!(
        "ATTACH '{table}' AS db",
    );
    let m = concat!("SELECT 1", "{");
    let n = format!(
        "SELECT {}", id
    );
}
