//! Reads only a bounded public browser test vector, never application data.
use std::io::Read;
use vault_core::vault::{Header, Keys, Record};
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Vector {
    header: Header,
    record: Record,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    std::io::stdin().take(8193).read_to_end(&mut bytes)?;
    if bytes.len() > 8192 {
        return Err("vector limit".into());
    }
    let vector: Vector = serde_json::from_slice(&bytes)?;
    let keys = Keys::unlock(vector.header, "production vector test only")?;
    if keys
        .open("fixture-account-a", "fixture-record", &vector.record)?
        .as_slice()
        != b"M03 PRIVATE MESSAGE MARKER"
    {
        return Err("cross-runtime record mismatch".into());
    }
    println!("PASS browser production wrapper and record opened in native Rust");
    Ok(())
}
