use super::*;
struct Logs {
    dialog: adw::Dialog,
    stack: gtk::Stack,
    text: gtk::TextView,
    status: adw::StatusPage,
    refresh: gtk::Button,
    copy: gtk::Button,
    spinner: gtk::Spinner,
    busy: Cell<bool>,
    kind: Kind,
}
pub(super) fn logs_dialog(ui: &Rc<Ui>) {
    let dialog = adw::Dialog::builder()
        .title("Firewall Logs")
        .content_width(680)
        .content_height(520)
        .build();
    dialog.set_widget_name("firewall-logs");
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let refresh = icon_button("view-refresh-symbolic", "Refresh Logs");
    let copy = icon_button("edit-copy-symbolic", "Copy Logs");
    header.pack_start(&refresh);
    header.pack_end(&copy);
    let spinner = gtk::Spinner::new();
    header.pack_start(&spinner);
    toolbar.add_top_bar(&header);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let note=gtk::Label::new(Some("Up to 200 recent entries visible to your account. Logging and journal access depend on your system."));
    note.set_wrap(true);
    note.add_css_class("caption");
    note.add_css_class("dim-label");
    margins(&note, 12);
    body.append(&note);
    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    let status = adw::StatusPage::new();
    status.add_css_class("compact");
    stack.add_named(&status, Some("status"));
    let text = gtk::TextView::new();
    text.set_editable(false);
    text.set_monospace(true);
    text.set_wrap_mode(gtk::WrapMode::WordChar);
    text.set_left_margin(16);
    text.set_right_margin(16);
    text.set_top_margin(16);
    text.set_bottom_margin(16);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&text));
    stack.add_named(&scroll, Some("entries"));
    body.append(&stack);
    toolbar.set_content(Some(&body));
    dialog.set_child(Some(&toolbar));
    let logs = Rc::new(Logs {
        dialog,
        stack,
        text,
        status,
        refresh,
        copy,
        spinner,
        busy: Cell::new(false),
        kind: ui.kind.get(),
    });
    let weak = Rc::downgrade(&logs);
    logs.refresh.connect_clicked(move |_| {
        if let Some(logs) = weak.upgrade() {
            load(&logs);
        }
    });
    let u = ui.clone();
    let weak = Rc::downgrade(&logs);
    logs.copy.connect_clicked(move |_| {
        if let Some(logs) = weak.upgrade() {
            let b = logs.text.buffer();
            u.window
                .clipboard()
                .set_text(&b.text(&b.start_iter(), &b.end_iter(), false));
            u.toast("Firewall logs copied");
        }
    });
    // Keep the controller alive while the dialog is presented; release on close.
    let keep = Rc::new(RefCell::new(Some(logs.clone())));
    logs.dialog.connect_closed(move |_| {
        keep.borrow_mut().take();
    });
    logs.dialog.present(Some(&ui.window));
    load(&logs);
}
fn load(logs: &Rc<Logs>) {
    if logs.busy.replace(true) {
        return;
    }
    logs.refresh.set_sensitive(false);
    logs.copy.set_sensitive(false);
    logs.spinner.start();
    logs.status
        .set_icon_name(Some("document-open-recent-symbolic"));
    logs.status.set_title("Reading Firewall Logs");
    logs.status
        .set_description(Some("Checking entries available to your account…"));
    logs.stack.set_visible_child_name("status");
    let kind = logs.kind;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = if kind == Kind::Mock {
            Ok(String::new())
        } else {
            let mut command = std::process::Command::new("/usr/bin/journalctl");
            command.args(["--no-pager", "--quiet", "-n", "200", "-o", "short-iso"]);
            if kind == Kind::Firewalld {
                command.args(["-u", "firewalld"]);
            } else {
                command.args(["-k", "--grep=UFW"]);
            }
            command.output().map_err(|e| e.to_string()).and_then(|o| {
                if o.status.success() {
                    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
                } else {
                    Err(String::from_utf8_lossy(&o.stderr).into_owned())
                }
            })
        };
        let _ = tx.send(result);
    });
    let weak = Rc::downgrade(logs);
    glib::timeout_add_local(Duration::from_millis(100), move || {
        let Some(logs) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => Err("The journal connection stopped unexpectedly.".into()),
        };
        logs.busy.set(false);
        logs.spinner.stop();
        logs.refresh.set_sensitive(true);
        match result {
            Ok(s) if !s.trim().is_empty() => {
                logs.text.buffer().set_text(&s);
                logs.copy.set_sensitive(true);
                logs.stack.set_visible_child_name("entries");
            }
            Ok(_) => {
                logs.status.set_title("No Log Entries");
                logs.status
                    .set_icon_name(Some("document-open-recent-symbolic"));
                logs.status.set_description(Some(if logs.kind==Kind::Mock{"Preview mode does not read host logs."}else{"Logging may be disabled, no recent activity may be recorded, or journal access may be restricted."}));
            }
            Err(e) => {
                logs.status.set_title("Logs Are Unavailable");
                logs.status.set_icon_name(Some("dialog-warning-symbolic"));
                logs.status.set_description(Some(&format!(
                    "Check journal permissions and logging configuration, then refresh.\n{e}"
                )));
            }
        }
        glib::ControlFlow::Break
    });
}
