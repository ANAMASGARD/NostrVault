use std::{env, fs, path::Path};
use vault_native::age_proof::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        return Err("expected write|verify directory".into());
    }
    let path = Path::new(&args[2]);
    match args[1].as_str() {
        "write" => {
            fs::create_dir_all(path)?;
            fs::write(
                path.join("native-passphrase.age"),
                encrypt_passphrase(vault_core::ZIP_BYTES)?,
            )?;
            fs::write(
                path.join("native-recipient.age"),
                encrypt_recipient(vault_core::ZIP_BYTES, REFERENCE_RECIPIENT.trim())?,
            )?;
        }
        "verify" => {
            let pass = fs::read(path.join("wasm-passphrase.age"))?;
            let recipient = fs::read(path.join("wasm-recipient.age"))?;
            if decrypt_passphrase(&pass)? != vault_core::ZIP_BYTES
                || decrypt_recipient(&recipient, REFERENCE_IDENTITY.trim())?
                    != vault_core::ZIP_BYTES
            {
                return Err("interoperability mismatch".into());
            }
        }
        _ => return Err("unknown operation".into()),
    }
    Ok(())
}
