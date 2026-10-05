fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut timings = Vec::new();
    for run in 0..6 {
        let start = std::time::Instant::now();
        let _key = vault_core::vault::derive("benchmark test only", &[7; 16])?;
        if run > 0 {
            timings.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    println!(
        "{}",
        serde_json::json!({"profile":"argon2id-v19-65536-3-4","memoryKiB":65536,"warmups":1,"milliseconds":timings,"host":std::env::consts::OS})
    );
    Ok(())
}
