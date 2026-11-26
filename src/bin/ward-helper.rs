use ward::{backend::ufw, model::Request};
fn main() {
    // pkexec is the only installed authorization boundary; never install setuid.
    let result = (|| {
        let mut args = std::env::args().skip(1);
        let json = args.next().ok_or("Missing request")?;
        if json.len() > 65536 || args.next().is_some() {
            return Err("Invalid request size".to_string());
        }
        let request: Request = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        // Serialize helper clients; UFW itself also takes its own backend lock.
        let _lock = Lock::acquire()?;
        let snapshot = ufw::execute(request)?;
        serde_json::to_string(&snapshot).map_err(|e| e.to_string())
    })();
    match result {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
struct Lock;
impl Lock {
    fn acquire() -> Result<Self, String> {
        std::fs::create_dir("/run/ward-helper.lock").map_err(|_| {
            "Another Ward operation is running, or the helper lacks privileges.".to_string()
        })?;
        Ok(Self)
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir("/run/ward-helper.lock");
    }
}
