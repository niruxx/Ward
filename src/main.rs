use adw::prelude::*;
fn main() -> glib::ExitCode {
    if std::env::var("USER").as_deref() == Ok("root")
        || std::fs::read_to_string("/proc/self/status")
            .ok()
            .is_some_and(|s| {
                s.lines().any(|l| {
                    l.starts_with("Uid:") && l.split_whitespace().skip(1).any(|v| v == "0")
                })
            })
    {
        eprintln!("Ward's graphical application must not run as root.");
        return glib::ExitCode::FAILURE;
    }
    let app = adw::Application::builder()
        .application_id("io.github.ward.Ward")
        .build();
    app.connect_activate(ward::ui::build);
    app.run()
}
