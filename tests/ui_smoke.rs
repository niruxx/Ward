#![cfg(feature = "gui")]
use adw::prelude::*;
use std::time::{Duration, Instant};
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
fn named<T: IsA<gtk::Widget> + glib::object::ObjectType>(root: &gtk::Widget, name: &str) -> T {
    find(root, &|w| w.widget_name() == name)
        .unwrap_or_else(|| panic!("Missing widget {name}"))
        .downcast::<T>()
        .expect("Correct widget type")
}
fn button(root: &gtk::Widget, label: &str) -> gtk::Button {
    find(root, &|w| {
        w.clone()
            .downcast::<gtk::Button>()
            .is_ok_and(|b| b.label().as_deref() == Some(label))
    })
    .unwrap_or_else(|| panic!("Missing button {label}"))
    .downcast::<gtk::Button>()
    .unwrap()
}
fn tooltip(root: &gtk::Widget, label: &str) -> gtk::Button {
    find(root, &|w| w.tooltip_text().as_deref() == Some(label))
        .unwrap_or_else(|| panic!("Missing control {label}"))
        .downcast::<gtk::Button>()
        .unwrap()
}
fn pump() {
    let c = glib::MainContext::default();
    let deadline = Instant::now() + Duration::from_millis(280);
    while Instant::now() < deadline {
        while c.pending() {
            c.iteration(false);
        }
        std::thread::sleep(Duration::from_millis(4));
    }
}
#[allow(deprecated)]
fn capture(window: &adw::ApplicationWindow, name: &str) {
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
        .save_to_png(std::path::Path::new(&dir).join(format!("{name}.png")))
        .unwrap();
}

fn dialog(window: &adw::ApplicationWindow) -> gtk::Widget {
    window.visible_dialog().expect("Expected a dialog").upcast()
}
fn close_dialog(window: &adw::ApplicationWindow) {
    window.visible_dialog().expect("Expected a dialog").close();
    pump();
}
#[test]
#[ignore = "Requires a graphical session and GSettings schema; all mutations use the explicit mock backend"]
fn complete_native_ui_workflow() {
    std::env::set_var("WARD_MOCK", "1");
    std::env::set_var("GSETTINGS_BACKEND", "memory");
    adw::init().unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.ward.Ward.Test")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(gio::Cancellable::NONE).unwrap();
    ward::ui::build(&app);
    pump();
    let window = app
        .active_window()
        .unwrap()
        .downcast::<adw::ApplicationWindow>()
        .unwrap();
    let root = window.clone().upcast::<gtk::Widget>();
    let stack: gtk::Stack = named(&root, "main-pages");
    let style = adw::StyleManager::default();
    style.set_color_scheme(adw::ColorScheme::ForceLight);
    pump();
    capture(&window, "welcome-light-wide");
    style.set_color_scheme(adw::ColorScheme::ForceDark);
    pump();
    capture(&window, "welcome-dark-wide");
    style.set_color_scheme(adw::ColorScheme::ForceLight);
    window.set_default_size(390, 640);
    pump();
    capture(&window, "welcome-light-compact");
    assert!(
        window.width() <= 550,
        "Compact window should fit: {}",
        window.width()
    );
    style.set_color_scheme(adw::ColorScheme::ForceDark);
    pump();
    capture(&window, "welcome-dark-compact");
    for (label, next) in [
        ("Get Started", "detect"),
        ("Continue", "learn"),
        ("Continue", "ready"),
        ("Open Ward", "dashboard"),
    ] {
        button(&stack.visible_child().unwrap(), label).emit_clicked();
        pump();
        assert_eq!(stack.visible_child_name().as_deref(), Some(next));
        capture(&window, &format!("{next}-dark-compact"));
    }
    let dashboard = stack.visible_child().unwrap();
    let refresh: gtk::Button = named(&root, "refresh-firewall");
    refresh.emit_clicked();
    pump();
    capture(&window, "dashboard-dark-compact");
    window.set_default_size(1100, 800);
    style.set_color_scheme(adw::ColorScheme::ForceLight);
    pump();
    capture(&window, "dashboard-light-wide");
    style.set_color_scheme(adw::ColorScheme::ForceDark);
    pump();
    capture(&window, "dashboard-dark-wide");
    window.set_default_size(390, 640);
    style.set_color_scheme(adw::ColorScheme::ForceLight);
    pump();
    capture(&window, "dashboard-light-compact");
    window.set_default_size(1100, 800);
    pump();
    window.set_default_size(360, 600);
    pump();
    assert!(
        window.width() <= 360,
        "Minimum supported layout should fit: {}",
        window.width()
    );
    capture(&window, "dashboard-light-minimum");
    window.set_default_size(1100, 800);
    pump();
    let add: gtk::Button = named(&dashboard, "add-rule");
    assert!(add.is_sensitive());
    add.emit_clicked();
    pump();
    let editor = dialog(&window);
    let port: adw::EntryRow = named(&editor, "rule-port");
    let review: gtk::Button = named(&editor, "form-review");
    assert!(!review.is_sensitive());
    port.set_text("70000");
    port.select_region(0, 0);
    review.grab_focus();
    review.emit_clicked();
    pump();
    let error: gtk::Label = named(&editor, "form-error");
    assert!(error.is_visible());
    assert!(dialog(&window).is::<adw::Dialog>());
    port.set_text("443");
    port.select_region(0, 0);
    assert!(!error.is_visible());
    review.grab_focus();
    review.emit_clicked();
    pump();
    let alert = dialog(&window).downcast::<adw::AlertDialog>().unwrap();
    assert!(alert.body().contains("Destination: any"));
    button(&alert.upcast(), "Cancel").emit_clicked();
    pump();
    assert_eq!(port.text(), "443");
    assert_eq!(
        window.visible_dialog().unwrap().widget_name(),
        "rule-editor"
    );
    window.set_default_size(390, 640);
    pump();
    capture(&window, "rule-editor-light-compact");
    review.grab_focus();
    review.emit_clicked();
    pump();
    button(&dialog(&window), "Add Rule").emit_clicked();
    pump();
    let list: gtk::ListBox = named(&dashboard, "rule-list");
    assert!(list.first_child().is_some());
    let search: gtk::SearchEntry = named(&dashboard, "rule-search");
    search.set_text("no-such-rule");
    pump();
    let empty: gtk::Label = named(&dashboard, "empty-title");
    assert_eq!(empty.text(), "No Matching Rules");
    capture(&window, "search-empty-light-compact");
    button(&dashboard, "Clear Search").emit_clicked();
    pump();
    assert!(search.text().is_empty());
    tooltip(&list.clone().upcast(), "Delete Rule").emit_clicked();
    pump();
    button(&dialog(&window), "Cancel").emit_clicked();
    pump();
    assert!(list.first_child().is_some());
    tooltip(&list.upcast(), "Delete Rule").emit_clicked();
    pump();
    button(&dialog(&window), "Delete Rule").emit_clicked();
    pump();
    assert_eq!(empty.text(), "No Rules to Show");
    // A home-page port exception takes the short path to the same validated backend.
    let allow: gtk::Button = named(&root, "compact-allow-port");
    assert!(allow.is_sensitive());
    allow.emit_clicked();
    pump();
    let form = dialog(&window);
    let port: adw::EntryRow = named(&form, "exception-port");
    let review: gtk::Button = named(&form, "form-review");
    assert!(!review.is_sensitive());
    port.set_text("0");
    review.grab_focus();
    review.emit_clicked();
    pump();
    let error: gtk::Label = named(&form, "form-error");
    assert!(error.is_visible());
    port.set_text("8080");
    let protocol: adw::ComboRow = named(&form, "exception-protocol");
    protocol.set_selected(1);
    let audience: adw::ComboRow = named(&form, "exception-audience");
    audience.set_selected(1);
    let source: adw::EntryRow = named(&form, "exception-source");
    assert!(source.is_visible());
    source.set_text("invalid");
    review.grab_focus();
    review.emit_clicked();
    pump();
    assert!(error.is_visible());
    source.set_text("192.0.2.0/24");
    review.grab_focus();
    review.emit_clicked();
    pump();
    let alert = dialog(&window).downcast::<adw::AlertDialog>().unwrap();
    assert!(alert.body().contains("UDP"));
    assert!(alert.body().contains("192.0.2.0/24"));
    button(&alert.upcast(), "Cancel").emit_clicked();
    pump();
    assert_eq!(port.text(), "8080");
    capture(&window, "port-editor-light-compact");
    review.grab_focus();
    review.emit_clicked();
    pump();
    button(&dialog(&window), "Allow Port").emit_clicked();
    pump();
    let list: gtk::ListBox = named(&dashboard, "rule-list");
    assert!(list.first_child().is_some());
    capture(&window, "home-with-port-exception");
    tooltip(&list.upcast(), "Delete Rule").emit_clicked();
    pump();
    button(&dialog(&window), "Delete Rule").emit_clicked();
    pump();
    button(&dashboard, "Enable Firewall").emit_clicked();
    pump();
    button(&dialog(&window), "Enable Firewall").emit_clicked();
    pump();
    assert!(button(&dashboard, "Disable Firewall").is_sensitive());
    window.lookup_action("preferences").unwrap().activate(None);
    pump();
    let prefs = dialog(&window);
    let animations: adw::SwitchRow = named(&prefs, "animation-choice");
    let theme: adw::ComboRow = named(&prefs, "style-choice");
    theme.set_selected(2);
    pump();
    assert_eq!(style.color_scheme(), adw::ColorScheme::ForceDark);
    capture(&window, "preferences-dark-compact");
    theme.set_selected(0);
    assert_eq!(style.color_scheme(), adw::ColorScheme::Default);
    animations.set_active(false);
    assert_eq!(stack.transition_duration(), 0);
    let desktop = gtk::Settings::default().unwrap();
    let original = desktop.is_gtk_enable_animations();
    desktop.set_gtk_enable_animations(false);
    animations.set_active(true);
    assert_eq!(stack.transition_duration(), 0);
    desktop.set_gtk_enable_animations(original);
    theme.set_selected(1);
    pump();
    assert_eq!(style.color_scheme(), adw::ColorScheme::ForceLight);
    close_dialog(&window);
    button(&dashboard, "View Logs").emit_clicked();
    pump();
    capture(&window, "logs-light-compact");
    assert!(!tooltip(&dialog(&window), "Copy Logs").is_sensitive());
    tooltip(&dialog(&window), "Refresh Logs").emit_clicked();
    pump();
    close_dialog(&window);
    window.lookup_action("diagnostics").unwrap().activate(None);
    pump();
    assert!(tooltip(&dialog(&window), "Save Report").is_sensitive());
    tooltip(&dialog(&window), "Copy Report").emit_clicked();
    close_dialog(&window);
    for action in ["shortcuts", "about"] {
        window.lookup_action(action).unwrap().activate(None);
        pump();
        assert!(window.visible_dialog().is_some());
        close_dialog(&window);
    }
    window.lookup_action("introduction").unwrap().activate(None);
    pump();
    assert_eq!(stack.visible_child_name().as_deref(), Some("welcome"));
    tooltip(&root, "Return to Firewall").emit_clicked();
    pump();
    assert_eq!(stack.visible_child_name().as_deref(), Some("dashboard"));
    window.close();
    pump();
}
