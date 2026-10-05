//! Public synthetic fixtures, run by verify-vault.mjs in isolated processes.
use vault_core::vault::{Operation, Request, State};
use vault_native::vault::Runtime;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let phase = args.get(1).ok_or("phase")?;
    let directory = std::path::PathBuf::from(args.get(2).ok_or("directory")?);
    let runtime = Runtime::new(directory)?;
    let mut request = Request {
        version: 1,
        request_id: "process-proof".into(),
        token: String::new(),
        generation: 0,
        vault_id: None,
        operation: Operation::Status,
    };
    let status = runtime.execute(&request)?;
    request.token = status.token;
    request.generation = status.generation;
    request.vault_id = status.vault_id;
    request.operation = match phase.as_str() {
        "create" => Operation::Create {
            password: "process fixture password".into(),
            confirmation: "process fixture password".into(),
        },
        "reopen" | "change" => Operation::Unlock {
            password: "process fixture password".into(),
        },
        "new-password" => Operation::Unlock {
            password: "changed fixture password".into(),
        },
        _ => return Err("unknown phase".into()),
    };
    let status = runtime.execute(&request)?;
    if status.state != State::Unlocked || status.setup.is_none() {
        return Err("setup not recovered".into());
    }
    if phase == "change" {
        request.token = status.token;
        request.generation = status.generation;
        request.vault_id = status.vault_id;
        request.operation = Operation::ChangePassword {
            current: "process fixture password".into(),
            password: "changed fixture password".into(),
            confirmation: "changed fixture password".into(),
        };
        if runtime.execute(&request)?.state != State::Locked {
            return Err("change did not lock".into());
        }
    }
    println!("PASS {phase}: committed production storage in independent process");
    Ok(())
}
