use super::*;
fn pump() {
    let c = glib::MainContext::default();
    let end = std::time::Instant::now() + Duration::from_millis(240);
    while std::time::Instant::now() < end {
        while c.pending() {
            c.iteration(false);
        }
        std::thread::sleep(Duration::from_millis(4));
    }
}
fn find(root: &gtk::Widget, predicate: &impl Fn(&gtk::Widget) -> bool) -> Option<gtk::Widget> {
    if predicate(root) {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(w) = child {
        if let Some(found) = find(&w, predicate) {
            return Some(found);
        }
        child = w.next_sibling();
    }
    None
}
fn row(root: &gtk::Widget, title: &str) -> adw::ActionRow {
    find(root, &|w| {
        w.clone()
            .downcast::<adw::ActionRow>()
            .is_ok_and(|r| r.title() == title)
    })
    .unwrap_or_else(|| panic!("Missing action {title}"))
    .downcast()
    .unwrap()
}
fn button(root: &gtk::Widget, label: &str) -> gtk::Button {
    find(root, &|w| {
        w.clone()
            .downcast::<gtk::Button>()
            .is_ok_and(|b| b.label().as_deref() == Some(label))
    })
    .unwrap_or_else(|| panic!("Missing button {label}"))
    .downcast()
    .unwrap()
}
fn entry(root: &gtk::Widget, title: &str) -> adw::EntryRow {
    find(root, &|w| {
        w.clone()
            .downcast::<adw::EntryRow>()
            .is_ok_and(|r| r.title() == title)
    })
    .unwrap()
    .downcast()
    .unwrap()
}
#[allow(deprecated)]
fn capture(u: &Ui, name: &str) {
    let window = &u.window;
    let Some(dir) = std::env::var_os("WARD_UI_CAPTURE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    // GTK 4.24 / libadwaita 1.10 may put dialogs on a separate native surface.
    // Snapshot the visible content and include its native theme background.
    let modal = window.visible_dialog();
    let widget = modal
        .as_ref()
        .and_then(|d| d.child())
        .or_else(|| window.content())
        .expect("Visible content");
    let paintable = gtk::WidgetPaintable::new(Some(&widget));
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let node = loop {
        let s = gtk::Snapshot::new();
        paintable.snapshot(&s, f64::from(widget.width()), f64::from(widget.height()));
        if let Some(node) = s.to_node() {
            break node;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Content did not render for screenshot {name}"
        );
        widget.queue_draw();
        pump();
    };
    let dark = adw::StyleManager::default().is_dark();
    let background = widget
        .style_context()
        .lookup_color(if modal.is_some() {
            "dialog_bg_color"
        } else {
            "window_bg_color"
        })
        .unwrap_or_else(|| {
            gtk::gdk::RGBA::parse(if dark { "#222226" } else { "#fafafb" }).unwrap()
        });
    let rect = gtk::graphene::Rect::new(0.0, 0.0, widget.width() as f32, widget.height() as f32);
    let s = gtk::Snapshot::new();
    s.append_color(&background, &rect);
    s.append_node(&node);
    let node = s.to_node().unwrap();
    window
        .renderer()
        .unwrap()
        .render_texture(&node, Some(&rect))
        .save_to_png(std::path::Path::new(&dir).join(format!(
            "form-{}.png",
            name.to_lowercase().replace(' ', "-")
        )))
        .unwrap();
}

fn current(u: &Ui) -> gtk::Widget {
    u.window.visible_dialog().unwrap().upcast()
}
fn close(u: &Ui) {
    u.window.visible_dialog().unwrap().close();
    pump();
}
#[test]
#[ignore = "Requires a graphical session; backend-specific forms are inspected and canceled without backend calls"]
fn backend_forms_and_feedback() {
    std::env::set_var("WARD_MOCK", "1");
    std::env::set_var("GSETTINGS_BACKEND", "memory");
    adw::init().unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.ward.Ward.FormsTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(gio::Cancellable::NONE).unwrap();
    let u = build_window(&app);
    pump();
    u.navigate("dashboard");
    // The only executed backend remains Mock. Switching this UI capability value
    // lets us inspect real forms; every real-backend confirmation is canceled.
    let revision = u.snapshot.borrow().as_ref().unwrap().revision.clone();
    for kind in [Kind::Ufw, Kind::Firewalld] {
        u.kind.set(kind);
        settings::preferences(&u);
        pump();
        let prefs = u
            .window
            .visible_dialog()
            .unwrap()
            .downcast::<adw::PreferencesDialog>()
            .unwrap();
        prefs.set_visible_page_name("firewall");
        let titles = if kind == Kind::Ufw {
            vec![
                "Incoming Policy",
                "Outgoing Policy",
                "Logging Level",
                "Allow Application Profile",
            ]
        } else {
            vec![
                "Allow Service",
                "Zone Membership",
                "Save Runtime Configuration",
                "Reload Permanent Configuration",
            ]
        };
        for title in titles {
            let action = row(&prefs.clone().upcast(), title);
            assert!(action.is_sensitive());
            action.emit_by_name::<()>("activated", &[]);
            pump();
            let form = current(&u);
            capture(&u, title);
            if let Ok(alert) = form.clone().downcast::<adw::AlertDialog>() {
                assert_eq!(alert.default_response().as_deref(), Some("cancel"));
                button(&form, "Cancel").emit_clicked();
                pump();
            } else {
                let review = button(&form, "Review");
                if ["Incoming Policy", "Outgoing Policy", "Logging Level"].contains(&title) {
                    assert!(!review.is_sensitive());
                    let selection = row(&form, title).downcast::<adw::ComboRow>().unwrap();
                    selection.set_selected(1);
                    assert!(review.is_sensitive());
                    review.grab_focus();
                    review.emit_clicked();
                    pump();
                    button(&current(&u), "Cancel").emit_clicked();
                    pump();
                    assert!(u.window.visible_dialog().is_some());
                } else {
                    assert!(!review.is_sensitive());
                    if title == "Zone Membership" {
                        entry(&form, "Zone").set_text("public");
                        entry(&form, "Interface Name").set_text("eth0");
                    } else {
                        entry(
                            &form,
                            if title == "Allow Service" {
                                "Service Name"
                            } else {
                                "Application Profile"
                            },
                        )
                        .set_text(if title == "Allow Service" {
                            "ssh"
                        } else {
                            "OpenSSH"
                        });
                    }
                    assert!(review.is_sensitive());
                    review.grab_focus();
                    review.emit_clicked();
                    pump();
                    assert!(current(&u).is::<adw::AlertDialog>());
                    button(&current(&u), "Cancel").emit_clicked();
                    pump();
                    assert!(!current(&u).is::<adw::AlertDialog>());
                    if title == "Zone Membership" {
                        let choice = row(&form, "Change").downcast::<adw::ComboRow>().unwrap();
                        choice.set_selected(2);
                        let value = entry(&form, "Source IP or Network");
                        value.set_text("invalid");
                        review.grab_focus();
                        review.emit_clicked();
                        pump();
                        assert!(!current(&u).is::<adw::AlertDialog>());
                        value.set_text("2001:db8::/64");
                        review.grab_focus();
                        review.emit_clicked();
                        pump();
                        assert!(current(&u).is::<adw::AlertDialog>());
                        button(&current(&u), "Cancel").emit_clicked();
                        pump();
                    }
                }
                close(&u);
            }
            assert_eq!(u.snapshot.borrow().as_ref().unwrap().revision, revision);
        }
        close(&u);
        if kind == Kind::Firewalld {
            rules::rule_dialog(&u);
            pump();
            let form = current(&u);
            entry(&form, "Port or Range").set_text("8443");
            row(&form, "Action")
                .downcast::<adw::ComboRow>()
                .unwrap()
                .set_selected(1);
            entry(&form, "Source").set_text("192.0.2.0/24");
            entry(&form, "Destination").set_text("2001:db8::1");
            let review = button(&form, "Review");
            review.grab_focus();
            review.emit_clicked();
            pump();
            assert!(!current(&u).is::<adw::AlertDialog>());
            entry(&form, "Destination").set_text("any");
            review.grab_focus();
            review.emit_clicked();
            pump();
            let alert = current(&u).downcast::<adw::AlertDialog>().unwrap();
            assert!(alert.body().contains("192.0.2.0/24"));
            button(&alert.upcast(), "Cancel").emit_clicked();
            pump();
            assert_eq!(entry(&current(&u), "Source").text(), "192.0.2.0/24");
            close(&u);
        }
        rules::port_dialog(&u);
        pump();
        let form = current(&u);
        let port = entry(&form, "Port or Range");
        port.set_text("8443");
        let review = button(&form, "Review");
        review.grab_focus();
        review.emit_clicked();
        pump();
        let alert = current(&u).downcast::<adw::AlertDialog>().unwrap();
        assert!(alert.body().contains(if kind == Kind::Firewalld {
            "Runtime only"
        } else {
            "Saved by UFW"
        }));
        button(&alert.upcast(), "Cancel").emit_clicked();
        pump();
        assert_eq!(port.text(), "8443");
        close(&u);
    }
    // Known values are displayed as last-read state; unchanged choices cannot be submitted.
    u.kind.set(Kind::Ufw);
    u.snapshot.borrow_mut().as_mut().unwrap().details =
        "Logging: on (low)\nDefault: deny (incoming), allow (outgoing), disabled (routed)\n".into();
    settings::preferences(&u);
    pump();
    let prefs = u
        .window
        .visible_dialog()
        .unwrap()
        .downcast::<adw::PreferencesDialog>()
        .unwrap();
    prefs.set_visible_page_name("firewall");
    for (title, selected) in [
        ("Incoming Policy", 2),
        ("Outgoing Policy", 1),
        ("Logging Level", 2),
    ] {
        row(&prefs.clone().upcast(), title).emit_by_name::<()>("activated", &[]);
        pump();
        let form = current(&u);
        let choice = row(&form, title).downcast::<adw::ComboRow>().unwrap();
        assert_eq!(choice.selected(), selected);
        assert!(!button(&form, "Review").is_sensitive());
        close(&u);
    }
    close(&u);
    // Unknown state disables all backend mutation rows, rather than ignoring input.
    u.snapshot.replace(None);
    u.kind.set(Kind::Ufw);
    settings::preferences(&u);
    pump();
    assert!(!row(&current(&u), "Incoming Policy").is_sensitive());
    close(&u);
    // Backend-specific errors must invalidate state and expose recovery guidance.
    u.kind.set(Kind::Mock);
    u.run(Request::Status {});
    pump();
    let revision = u.snapshot.borrow().as_ref().unwrap().revision.clone();
    u.run(Request::Logging {
        level: "low".into(),
        expected: revision,
    });
    pump();
    assert!(u.snapshot.borrow().is_none());
    assert!(u.banner.is_revealed());
    capture(&u, "connection-error");
    assert!(u.detail.text().contains("may have been applied"));
    assert!(!u.add.is_sensitive());
    u.run(Request::Status {});
    pump();
    assert!(!u.banner.is_revealed());
    assert!(u.add.is_sensitive());
    u.window.close();
    pump();
}
