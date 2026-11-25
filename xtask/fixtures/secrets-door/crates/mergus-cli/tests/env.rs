//! Reads a secret variable outside `mergus-security`.

#[test]
fn reads() {
    let _ = std::env::var("MERGUS_SECRET_TOKEN");
}
