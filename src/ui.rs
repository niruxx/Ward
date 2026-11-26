mod home;
mod logs;
mod onboarding;
mod rules;
mod settings;
use crate::{
    backend::{self, Kind},
    model::*,
};
use adw::prelude::*;
use logs::logs_dialog;
use onboarding::onboarding;
use rules::{confirm, port_dialog, rule_dialog};
use settings::{apply_theme, preferences};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
const ID: &str = "io.github.ward.Ward";
struct Ui {
    window: adw::ApplicationWindow,
    overlay: adw::ToastOverlay,
    last_toast: RefCell<Option<adw::Toast>>,
    stack: gtk::Stack,
    settings: Option<gio::Settings>,
    kind: Cell<Kind>,
    snapshot: RefCell<Option<Snapshot>>,
    busy: Cell<bool>,
    animations: Cell<bool>,
    desktop: gtk::Settings,
    status: StatusSummary,
    detail: gtk::Label,
    backend_row: adw::ActionRow,
    count_row: adw::ActionRow,
    details_row: adw::ExpanderRow,
    banner: adw::Banner,
    refresh: gtk::Button,
    toggle: gtk::Button,
    add: gtk::Button,
    allow_port: gtk::Button,
    read_status: gtk::Button,
    home_help: gtk::Label,
    add_description: gtk::Label,
    incoming: adw::ActionRow,
    outgoing: adw::ActionRow,
    rules: gtk::ListBox,
    rule_group: adw::PreferencesGroup,
    rule_stack: gtk::Stack,
    empty: StatusSummary,
    clear_search: gtk::Button,
    search: gtk::SearchEntry,
    sort: gtk::DropDown,
    spinner: gtk::Spinner,
    pending: gtk::Label,
    detection: gtk::Label,
    backend_picker: gtk::DropDown,
    check_backend: gtk::Button,
    refreshed: gtk::Label,
    ready_summary: gtk::Label,
    return_button: gtk::Button,
    completed: Cell<bool>,
    mutations: RefCell<Vec<glib::WeakRef<gtk::Widget>>>,
    selectors: RefCell<Vec<glib::WeakRef<gtk::Widget>>>,
    add_action: gio::SimpleAction,
    port_action: gio::SimpleAction,
    compact_actions: gtk::ActionBar,
}
pub fn build(app: &adw::Application) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }
    build_window(app);
}
fn build_window(app: &adw::Application) -> Rc<Ui> {
    static RESOURCES: std::sync::Once = std::sync::Once::new();
    RESOURCES.call_once(|| {
        gio::resources_register_include!("ward.gresource").expect("Built-in application resources")
    });
    let settings = gio::SettingsSchemaSource::default()
        .and_then(|s| s.lookup(ID, true))
        .map(|s| gio::Settings::new_full(&s, None::<&gio::SettingsBackend>, None));
    let mock = std::env::var_os("WARD_MOCK").is_some();
    let kind = if mock {
        Kind::Mock
    } else if settings
        .as_ref()
        .is_some_and(|s| s.string("backend") == "firewalld")
    {
        Kind::Firewalld
    } else {
        Kind::Ufw
    };
    let completed = settings
        .as_ref()
        .is_some_and(|s| s.boolean("onboarding-complete"));
    let animations = settings.as_ref().is_none_or(|s| s.boolean("animations"));
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Ward")
        .default_width(840)
        .default_height(760)
        .width_request(360)
        .height_request(360)
        .build();
    gtk::IconTheme::for_display(&gtk::prelude::WidgetExt::display(&window))
        .add_resource_path("/io/github/ward/Ward/icons");
    let overlay = adw::ToastOverlay::new();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("Ward", "")));
    let return_button = icon_button("go-previous-symbolic", "Return to Firewall");
    return_button.set_visible(false);
    header.pack_start(&return_button);
    let refresh = icon_button("view-refresh-symbolic", "Refresh Firewall (Ctrl+R)");
    refresh.set_widget_name("refresh-firewall");
    header.pack_start(&refresh);
    let menu = gio::Menu::new();
    menu.append(Some("Preferences"), Some("win.preferences"));
    menu.append(Some("Introduction"), Some("win.introduction"));
    menu.append(Some("Keyboard Shortcuts"), Some("win.shortcuts"));
    let secondary = gio::Menu::new();
    secondary.append(Some("Diagnostic Report"), Some("win.diagnostics"));
    secondary.append(Some("About Ward"), Some("win.about"));
    menu.append_section(None, &secondary);
    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .tooltip_text("Main Menu")
        .primary(true)
        .build();
    menu_button.update_property(&[gtk::accessible::Property::Label("Main Menu")]);
    header.pack_end(&menu_button);
    toolbar.add_top_bar(&header);
    let banner = adw::Banner::builder().button_label("Details").build();
    toolbar.add_top_bar(&banner);
    let activity = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    activity.set_halign(gtk::Align::Center);
    margins(&activity, 6);
    let spinner = gtk::Spinner::new();
    let pending = gtk::Label::new(None);
    pending.set_wrap(true);
    pending.add_css_class("caption");
    activity.append(&spinner);
    activity.append(&pending);
    activity.set_visible(false);
    toolbar.add_top_bar(&activity);
    let compact_actions = gtk::ActionBar::new();
    compact_actions.set_revealed(false);
    compact_actions.set_visible(completed);
    let compact_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    compact_buttons.set_homogeneous(true);
    let compact_add = gtk::Button::with_label("Add Rule");
    compact_add.set_widget_name("compact-add-rule");
    compact_add.set_action_name(Some("win.add-rule"));
    let compact_port = gtk::Button::with_label("Allow Port");
    compact_port.set_widget_name("compact-allow-port");
    compact_port.set_action_name(Some("win.allow-port"));
    compact_port.add_css_class("suggested-action");
    compact_buttons.append(&compact_add);
    compact_buttons.append(&compact_port);
    compact_actions.set_center_widget(Some(&compact_buttons));
    toolbar.add_bottom_bar(&compact_actions);

    let stack = gtk::Stack::new();
    stack.set_widget_name("main-pages");
    stack.set_vexpand(true);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 20);
    margins(&content, 18);
    content.append(&home::heading(
        "Your Firewall",
        "See what’s happening. Choose what can connect.",
    ));
    let status = StatusSummary::new(
        "Let’s Check Your Firewall",
        "Read its current status before adding rules or allowing a port.",
        "security-medium-symbolic",
        false,
    );
    let toggle = gtk::Button::with_label("Enable Firewall");
    toggle.set_halign(gtk::Align::Start);
    toggle.add_css_class("pill");
    let read_status = gtk::Button::with_label("Read Firewall Status");
    read_status.add_css_class("suggested-action");
    read_status.set_widget_name("read-firewall");
    let status_controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    status_controls.append(&read_status);
    status_controls.append(&toggle);
    status.content.append(&status_controls);
    margins(&status.content, 18);
    let status_card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    status_card.add_css_class("card");
    status_card.append(&status.content);
    content.append(&status_card);
    let quick = gtk::Box::new(gtk::Orientation::Vertical, 10);
    let quick_title = gtk::Label::new(Some("Make a Change"));
    quick_title.set_xalign(0.0);
    quick_title.add_css_class("heading");
    quick.append(&quick_title);
    let quick_cards = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    quick_cards.set_homogeneous(true);
    let (add, add_description) = home::action_card(
        "Add Rule",
        "Choose the traffic, action, and addresses your firewall supports.",
        "list-add-symbolic",
    );
    add.set_widget_name("add-rule");
    let (allow_port, _) = home::action_card(
        "Allow Port",
        "Let an app receive connections on a TCP or UDP port.",
        "network-transmit-receive-symbolic",
    );
    allow_port.set_widget_name("allow-port");
    quick_cards.append(&add);
    quick_cards.append(&allow_port);
    quick.append(&quick_cards);
    let home_help = gtk::Label::new(Some(
        "Read your firewall status to start adding rules and port exceptions.",
    ));
    home_help.set_xalign(0.0);
    home_help.set_wrap(true);
    home_help.add_css_class("caption");
    home_help.add_css_class("dim-label");
    quick.append(&home_help);
    content.append(&quick);
    let overview = adw::PreferencesGroup::builder()
        .title("How Connections Are Handled")
        .description("Incoming connections reach this computer. Outgoing connections leave it.")
        .build();
    let backend_row = adw::ActionRow::builder()
        .title("Managed With")
        .subtitle(kind.label())
        .build();
    backend_row.add_prefix(&gtk::Image::from_icon_name("network-workgroup-symbolic"));
    let count_row = adw::ActionRow::builder()
        .title("Rules and Exceptions")
        .subtitle("Refresh to load configuration")
        .build();
    count_row.add_prefix(&gtk::Image::from_icon_name("view-list-symbolic"));
    let incoming = adw::ActionRow::builder()
        .title("Incoming Connections")
        .subtitle("Read status to see the reported policy")
        .build();
    incoming.add_prefix(&gtk::Image::from_icon_name("go-down-symbolic"));
    let outgoing = adw::ActionRow::builder()
        .title("Outgoing Connections")
        .subtitle("Read status to see the reported policy")
        .build();
    outgoing.add_prefix(&gtk::Image::from_icon_name("go-up-symbolic"));
    overview.add(&incoming);
    overview.add(&outgoing);
    overview.add(&backend_row);
    overview.add(&count_row);
    let details_row = adw::ExpanderRow::builder()
        .title("Configuration Details")
        .subtitle("Policies and configuration from your firewall")
        .build();
    let detail = gtk::Label::new(Some("No confirmed configuration yet."));
    detail.set_wrap(true);
    detail.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    detail.set_selectable(true);
    detail.set_xalign(0.0);
    detail.add_css_class("caption");
    margins(&detail, 16);
    details_row.add_row(&detail);
    overview.add(&details_row);

    let rule_group = adw::PreferencesGroup::builder()
        .title("Rules and Port Exceptions")
        .description("Read your firewall status to see what is configured")
        .build();
    let add_menu = gio::Menu::new();
    add_menu.append(Some("Add Rule"), Some("win.add-rule"));
    add_menu.append(Some("Allow Port"), Some("win.allow-port"));
    let list_add = gtk::MenuButton::builder()
        .icon_name("list-add-symbolic")
        .menu_model(&add_menu)
        .tooltip_text("Add a Rule or Port Exception")
        .valign(gtk::Align::Center)
        .build();
    list_add.add_css_class("flat");
    rule_group.set_header_suffix(Some(&list_add));
    let filters = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    filters.set_margin_bottom(12);
    let search = gtk::SearchEntry::new();
    search.set_widget_name("rule-search");
    search.set_placeholder_text(Some("Search rules"));
    search.set_hexpand(true);
    let sort = gtk::DropDown::from_strings(&["Backend Order", "Description", "Protocol", "Port"]);
    sort.set_tooltip_text(Some("Sort Rules"));
    sort.update_property(&[gtk::accessible::Property::Label("Sort Rules")]);
    filters.append(&search);
    filters.append(&sort);
    rule_group.add(&filters);
    let rules = gtk::ListBox::new();
    rules.set_selection_mode(gtk::SelectionMode::None);
    rules.add_css_class("boxed-list");
    rules.set_widget_name("rule-list");
    let empty = StatusSummary::new(
        "Your Rules Will Appear Here",
        "Refresh to read your firewall rules.",
        "view-list-symbolic",
        true,
    );
    empty.content.set_widget_name("rules-empty");
    empty.title.set_widget_name("empty-title");
    let clear_search = gtk::Button::with_label("Clear Search");
    clear_search.set_halign(gtk::Align::Center);
    empty.content.append(&clear_search);
    let rule_stack = gtk::Stack::new();
    rule_stack.add_named(&rules, Some("list"));
    rule_stack.add_named(&empty.content, Some("empty"));
    rule_group.add(&rule_stack);
    content.append(&rule_group);
    content.append(&overview);
    let footer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let logs = gtk::Button::with_label("View Logs");
    logs.add_css_class("flat");
    logs.set_halign(gtk::Align::Center);
    footer.append(&logs);
    let refreshed = gtk::Label::new(Some("Not refreshed yet"));
    refreshed.set_wrap(true);
    refreshed.add_css_class("caption");
    refreshed.add_css_class("dim-label");
    refreshed.set_xalign(0.0);
    status.content.append(&refreshed);
    let note = gtk::Label::new(Some(
        "A firewall controls network traffic. It does not guarantee system security.",
    ));
    note.set_wrap(true);
    note.set_justify(gtk::Justification::Center);
    note.add_css_class("caption");
    note.add_css_class("dim-label");
    footer.append(&note);
    content.append(&footer);
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(860);
    clamp.set_tightening_threshold(560);
    clamp.set_child(Some(&content));
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&clamp));
    stack.add_named(&scroll, Some("dashboard"));
    let compact = adw::Breakpoint::new(
        adw::BreakpointCondition::parse("max-width: 550sp").expect("Valid breakpoint"),
    );
    compact.add_setter(
        &filters,
        "orientation",
        Some(&gtk::Orientation::Vertical.to_value()),
    );
    compact.add_setter(
        &quick_cards,
        "orientation",
        Some(&gtk::Orientation::Vertical.to_value()),
    );
    compact.add_setter(&compact_actions, "revealed", Some(&true.to_value()));
    compact.add_setter(&quick_cards, "visible", Some(&false.to_value()));
    compact.add_setter(&quick_title, "visible", Some(&false.to_value()));
    window.add_breakpoint(compact);
    let detection = gtk::Label::new(Some("Looking for available firewalls…"));
    detection.set_wrap(true);
    detection.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    detection.set_xalign(0.0);
    let backend_picker = gtk::DropDown::from_strings(if mock {
        &["Preview Mode"]
    } else {
        &["UFW", "firewalld"]
    });
    backend_picker.set_selected(if kind == Kind::Firewalld { 1 } else { 0 });
    backend_picker.set_tooltip_text(Some("Firewall Backend"));
    backend_picker.update_property(&[gtk::accessible::Property::Label("Firewall Backend")]);
    let check_backend = gtk::Button::with_label("Check Firewall Status");
    check_backend.set_halign(gtk::Align::Center);
    let ready_summary = gtk::Label::new(Some(
        "Live firewall status has not been checked. You can refresh it from the dashboard.",
    ));
    ready_summary.set_wrap(true);
    ready_summary.set_justify(gtk::Justification::Center);
    let add_action = gio::SimpleAction::new("add-rule", None);
    window.add_action(&add_action);
    let port_action = gio::SimpleAction::new("allow-port", None);
    window.add_action(&port_action);
    let ui = Rc::new(Ui {
        window,
        overlay,
        last_toast: RefCell::new(None),
        stack,
        settings,
        kind: Cell::new(kind),
        snapshot: RefCell::new(None),
        busy: Cell::new(false),
        animations: Cell::new(animations),
        desktop: gtk::Settings::default().expect("GTK initialized"),
        status,
        detail,
        backend_row,
        count_row,
        details_row,
        banner,
        refresh,
        toggle,
        add,
        allow_port,
        read_status,
        home_help,
        add_description,
        incoming,
        outgoing,
        rules,
        rule_group,
        rule_stack,
        empty,
        clear_search,
        search,
        sort,
        spinner,
        pending,
        detection,
        backend_picker,
        check_backend,
        refreshed,
        ready_summary,
        return_button,
        completed: Cell::new(completed),
        mutations: RefCell::new(vec![]),
        selectors: RefCell::new(vec![]),
        add_action,
        port_action,
        compact_actions,
    });
    onboarding(&ui);
    toolbar.set_content(Some(&ui.stack));
    ui.overlay.set_child(Some(&toolbar));
    ui.window.set_content(Some(&ui.overlay));
    apply_theme(&ui);
    ui.update_motion();
    let weak = Rc::downgrade(&ui);
    ui.desktop.connect_gtk_enable_animations_notify(move |_| {
        if let Some(u) = weak.upgrade() {
            u.update_motion();
        }
    });
    let u = ui.clone();
    ui.refresh
        .connect_clicked(move |_| u.run(Request::Status {}));
    let u = ui.clone();
    ui.check_backend
        .connect_clicked(move |_| u.run(Request::Status {}));
    let u = ui.clone();
    ui.add.connect_clicked(move |_| rule_dialog(&u));
    let u = ui.clone();
    ui.allow_port.connect_clicked(move |_| port_dialog(&u));
    let u = ui.clone();
    ui.read_status
        .connect_clicked(move |_| u.run(Request::Status {}));
    let u = ui.clone();
    ui.port_action.connect_activate(move |_, _| port_dialog(&u));
    let u = ui.clone();
    ui.add_action.connect_activate(move |_, _| rule_dialog(&u));
    let u = ui.clone();
    ui.toggle.connect_clicked(move |_|{let s=u.snapshot.borrow().clone();if let Some(s)=s{let enabled=!s.active;confirm(&u,if enabled{"Enable Firewall?"}else{"Disable Firewall?"},"This can interrupt SSH and other remote connections. Make sure you have local access or a recovery path.",Request::SetEnabled{enabled,expected:s.revision},None);}});
    let u = ui.clone();
    logs.connect_clicked(move |_| logs_dialog(&u));
    let u = ui.clone();
    ui.search.connect_search_changed(move |_| u.render_rules());
    let u = ui.clone();
    ui.sort.connect_selected_notify(move |_| u.render_rules());
    let u = ui.clone();
    ui.clear_search.connect_clicked(move |_| {
        u.search.set_text("");
        u.search.grab_focus();
    });
    let u = ui.clone();
    ui.return_button
        .connect_clicked(move |_| u.navigate("dashboard"));
    let u = ui.clone();
    ui.banner
        .connect_button_clicked(move |_| settings::show_details(&u));
    let u = ui.clone();
    ui.backend_picker.connect_selected_notify(move |p| {
        if u.kind.get() == Kind::Mock {
            return;
        }
        let kind = if p.selected() == 1 {
            Kind::Firewalld
        } else {
            Kind::Ufw
        };
        if kind == u.kind.get() {
            return;
        }
        u.kind.set(kind);
        u.snapshot.replace(None);
        u.banner.set_revealed(false);
        u.status.set_title("Let’s Check Your Firewall");
        u.incoming
            .set_subtitle("Read status to see the reported policy");
        u.outgoing
            .set_subtitle("Read status to see the reported policy");
        u.status.set_icon_name(Some("security-medium-symbolic"));
        u.status.set_description(Some(
            "Refresh to read the selected firewall’s state and rules.",
        ));
        u.toggle.set_label("Enable Firewall");
        u.detail.set_text("No confirmed configuration yet.");
        u.backend_row.set_subtitle(kind.label());
        u.count_row.set_subtitle("Refresh to load configuration");
        u.refreshed.set_text("Not refreshed yet");
        u.ready_summary.set_text(&format!(
            "Selected: {}. Live status has not been checked.",
            kind.label()
        ));
        u.render_rules();
        u.update_controls();
        u.save_string(
            "backend",
            if kind == Kind::Firewalld {
                "firewalld"
            } else {
                "ufw"
            },
        );
    });
    install_actions(&ui, app);
    let u = ui.clone();
    ui.stack.connect_visible_child_name_notify(move |_| {
        let dashboard = u.stack.visible_child_name().as_deref() == Some("dashboard");
        u.refresh.set_visible(dashboard);
        u.compact_actions.set_visible(dashboard);
        u.return_button.set_visible(!dashboard && u.completed.get());
    });
    ui.navigate(if completed { "dashboard" } else { "welcome" });
    ui.render_rules();
    ui.update_controls();
    ui.window.present();
    if ui.settings.is_none() {
        ui.toast("Preferences cannot be saved. The Ward settings schema is not installed.");
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(if mock {
            vec![Kind::Mock]
        } else {
            backend::detect()
        });
    });
    let u = ui.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || match rx.try_recv() {
        Ok(kinds) => {
            u.detection.set_text(&if kinds.is_empty(){"No supported firewall found. Install UFW or firewalld with your system’s package manager, then check its status.".into()}else{format!("Found: {}. Select the firewall you already use. Ward will not install or switch system services.",kinds.iter().map(|k|k.label()).collect::<Vec<_>>().join(", "))});
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(_) => {
            u.detection.set_text("Discovery was unavailable. Select a firewall and check its status to try connecting.");
            glib::ControlFlow::Break
        }
    });
    if mock || (completed && kind == Kind::Firewalld) {
        ui.run_with_feedback(Request::Status {}, false);
    }
    ui
}
fn install_actions(ui: &Rc<Ui>, app: &adw::Application) {
    type WindowAction = (&'static str, Box<dyn Fn()>);
    let actions: [WindowAction; 7] = [
        ("preferences", {
            let u = ui.clone();
            Box::new(move || preferences(&u))
        }),
        ("introduction", {
            let u = ui.clone();
            Box::new(move || u.navigate("welcome"))
        }),
        ("diagnostics", {
            let u = ui.clone();
            Box::new(move || settings::diagnostics(&u))
        }),
        ("about", {
            let u = ui.clone();
            Box::new(move || settings::about(&u))
        }),
        ("shortcuts", {
            let u = ui.clone();
            Box::new(move || settings::shortcuts(&u))
        }),
        ("refresh", {
            let u = ui.clone();
            Box::new(move || u.run(Request::Status {}))
        }),
        ("search", {
            let u = ui.clone();
            Box::new(move || {
                if u.stack.visible_child_name().as_deref() == Some("dashboard") {
                    u.search.grab_focus();
                }
            })
        }),
    ];
    for (name, callback) in actions {
        let action = gio::SimpleAction::new(name, None);
        let window = ui.window.downgrade();
        action.connect_activate(move |_, _| {
            if window
                .upgrade()
                .is_some_and(|w| w.visible_dialog().is_some())
            {
                return;
            }
            callback();
        });
        ui.window.add_action(&action);
    }
    for (name, keys) in [
        ("win.preferences", vec!["<Primary>comma"]),
        ("win.refresh", vec!["<Primary>r", "F5"]),
        ("win.add-rule", vec!["<Primary>n"]),
        ("win.search", vec!["<Primary>f"]),
    ] {
        app.set_accels_for_action(name, &keys);
    }
}
fn margins(w: &impl IsA<gtk::Widget>, n: i32) {
    w.set_margin_top(n);
    w.set_margin_bottom(n);
    w.set_margin_start(n);
    w.set_margin_end(n);
}
fn icon_button(icon: &str, label: &str) -> gtk::Button {
    let b = gtk::Button::from_icon_name(icon);
    b.set_tooltip_text(Some(label));
    b.update_property(&[gtk::accessible::Property::Label(label)]);
    b
}
impl Ui {
    fn toast(&self, text: &str) {
        if let Some(previous) = self.last_toast.borrow_mut().take() {
            previous.dismiss();
        }
        let toast = adw::Toast::new(text);
        toast.set_priority(adw::ToastPriority::High);
        self.overlay.add_toast(toast.clone());
        self.last_toast.replace(Some(toast));
    }
    fn save_string(&self, key: &str, value: &str) {
        if let Some(s) = &self.settings {
            if s.set_string(key, value).is_err() {
                self.toast("Your preference could not be saved.");
            }
        }
    }
    fn save_bool(&self, key: &str, value: bool) {
        if let Some(s) = &self.settings {
            if s.set_boolean(key, value).is_err() {
                self.toast("Your preference could not be saved.");
            }
        }
    }
    fn update_motion(&self) {
        let enabled = self.animations.get() && self.desktop.is_gtk_enable_animations();
        for s in [&self.stack, &self.rule_stack] {
            s.set_transition_type(if enabled {
                gtk::StackTransitionType::Crossfade
            } else {
                gtk::StackTransitionType::None
            });
            s.set_transition_duration(if enabled { 180 } else { 0 });
        }
    }
    fn navigate(&self, page: &str) {
        self.update_motion();
        let old = self.stack.visible_child_name();
        if self.animations.get()
            && self.desktop.is_gtk_enable_animations()
            && page != "dashboard"
            && old.as_deref() != Some("dashboard")
        {
            let order = ["welcome", "detect", "learn", "ready"];
            let before = old
                .as_deref()
                .and_then(|n| order.iter().position(|p| *p == n));
            let after = order.iter().position(|p| *p == page);
            self.stack.set_transition_type(if after > before {
                gtk::StackTransitionType::SlideLeft
            } else {
                gtk::StackTransitionType::SlideRight
            });
        }
        self.stack.set_visible_child_name(page);
        if page == "dashboard" {
            self.refresh.grab_focus();
        } else if let Some(child) = self.stack.visible_child() {
            focus_named(&child, "onboarding-next");
        }
    }
    fn track_mutation(&self, w: &impl IsA<gtk::Widget>) {
        self.mutations.borrow_mut().push(w.as_ref().downgrade());
        w.set_sensitive(!self.busy.get() && self.snapshot.borrow().is_some());
    }
    fn track_selector(&self, w: &impl IsA<gtk::Widget>) {
        self.selectors.borrow_mut().push(w.as_ref().downgrade());
        w.set_sensitive(!self.busy.get() && self.kind.get() != Kind::Mock);
    }
    fn update_controls(&self) {
        let ready = !self.busy.get();
        let known = self.snapshot.borrow().is_some();
        self.refresh.set_sensitive(ready);
        self.check_backend.set_sensitive(ready);
        self.check_backend.set_label(if ready {
            "Check Firewall Status"
        } else {
            "Checking Firewall…"
        });
        self.add.set_sensitive(ready && known);
        self.allow_port.set_sensitive(ready && known);
        self.port_action.set_enabled(ready && known);
        self.read_status.set_visible(!known);
        self.read_status.set_sensitive(ready);
        self.read_status.set_label(if ready {
            "Read Firewall Status"
        } else {
            "Reading Firewall…"
        });
        self.home_help.set_text(if !ready{"Checking the firewall. Creation actions will be available when its current state is confirmed."}else if !known {if self.kind.get()==Kind::Ufw{"Read UFW status with administrator authorization to start. Reading status does not change any rules."}else{"Read your firewall status to start adding rules and port exceptions."}}else{match self.kind.get(){Kind::Firewalld=>"Changes apply to firewalld at runtime. You can save them permanently from Preferences. Every change is reviewed first.",Kind::Ufw=>"UFW saves your changes. You will review each rule or port exception before it is applied.",Kind::Mock=>"Preview mode. Rules and port exceptions here do not change your computer’s firewall."}});
        self.add_description
            .set_text(if self.kind.get() == Kind::Firewalld {
                "Allow, block, or reject incoming traffic, with address restrictions."
            } else {
                "Choose what to allow, block, or limit, with address restrictions."
            });
        self.add_action.set_enabled(ready && known);
        self.toggle
            .set_visible(known && self.kind.get() != Kind::Firewalld);
        self.toggle.set_sensitive(ready && known);
        self.backend_picker
            .set_sensitive(ready && self.kind.get() != Kind::Mock);
        self.rules.set_sensitive(ready);
        for (collection, enabled) in [
            (&self.mutations, ready && known),
            (&self.selectors, ready && self.kind.get() != Kind::Mock),
        ] {
            collection.borrow_mut().retain(|weak| {
                if let Some(w) = weak.upgrade() {
                    w.set_sensitive(enabled);
                    true
                } else {
                    false
                }
            });
        }
    }
    fn run(self: &Rc<Self>, request: Request) {
        self.run_with_feedback(request, true);
    }
    fn run_with_feedback(self: &Rc<Self>, request: Request, notify: bool) {
        if self.busy.replace(true) {
            self.toast("Wait for the current firewall operation to finish.");
            return;
        }
        if let Some(previous) = self.last_toast.borrow_mut().take() {
            previous.dismiss();
        }
        let success = operation_success(&request);
        let mutation = !matches!(request, Request::Status {});
        let kind = self.kind.get();
        self.update_controls();
        self.spinner.start();
        self.pending.set_text(if mutation {
            "Applying change and checking firewall state…"
        } else if kind == Kind::Ufw {
            "Reading firewall state. Administrator authorization may be required…"
        } else {
            "Reading firewall state…"
        });
        if let Some(parent) = self.spinner.parent() {
            parent.set_visible(true);
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(backend::execute(kind, request));
        });
        let u = self.clone();
        glib::timeout_add_local(Duration::from_millis(80), move || {
            let result = match rx.try_recv() {
                Ok(r) => r,
                Err(std::sync::mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => {
                    Err("The firewall connection stopped unexpectedly. Try refreshing.".into())
                }
            };
            u.busy.set(false);
            u.spinner.stop();
            if let Some(parent) = u.spinner.parent() {
                parent.set_visible(false);
            }
            match result {
                Ok(s) => {
                    u.banner.set_revealed(false);
                    u.details_row.set_title("Configuration Details");
                    u.details_row
                        .set_subtitle("Policies and configuration from your firewall");
                    u.status.set_title(if s.active {
                        "Firewall Is On"
                    } else {
                        "Firewall Is Off"
                    });
                    u.status.set_icon_name(Some(if s.active {
                        "security-high-symbolic"
                    } else {
                        "security-low-symbolic"
                    }));
                    let explanation = if !s.active {
                        "This firewall is not filtering traffic. Stored rules are preserved; new rules take effect when you enable it.".into()
                    } else if kind == Kind::Firewalld {
                        format!("firewalld is running. New port exceptions will be added to the default zone: {}.",s.zone)
                    } else {
                        "Your firewall is running. Its rules and default policies determine which connections can get through.".into()
                    };
                    u.status.set_description(Some(&explanation));
                    u.toggle.set_label(if s.active {
                        "Disable Firewall"
                    } else {
                        "Enable Firewall"
                    });
                    u.toggle.remove_css_class("suggested-action");
                    if !s.active {
                        u.toggle.add_css_class("suggested-action");
                    }
                    u.backend_row.set_subtitle(&format!(
                        "{} · {}",
                        kind.label(),
                        if kind == Kind::Firewalld {
                            format!("Runtime · default zone {}", s.zone)
                        } else {
                            "Persistent configuration".into()
                        }
                    ));
                    u.count_row
                        .set_subtitle(&if kind == Kind::Ufw && !s.active {
                            "Stored rules are not listed while UFW is disabled".into()
                        } else {
                            format!(
                                "{} {}",
                                s.rules.len(),
                                if s.rules.len() == 1 {
                                    "entry reported by the firewall"
                                } else {
                                    "entries reported by the firewall"
                                }
                            )
                        });
                    match kind {
                        Kind::Ufw => {
                            u.incoming
                                .set_subtitle(&home::reported_policy(&s.details, "incoming"));
                            u.outgoing
                                .set_subtitle(&home::reported_policy(&s.details, "outgoing"));
                        }
                        Kind::Firewalld => {
                            u.incoming.set_subtitle(&format!(
                                "Controlled by zone rules · default zone {}",
                                s.zone
                            ));
                            u.outgoing.set_subtitle(
                                "Controlled by firewalld policies; see configuration details",
                            );
                        }
                        Kind::Mock => {
                            u.incoming
                                .set_subtitle("Preview only · host policy has not been read");
                            u.outgoing
                                .set_subtitle("Preview only · host policy has not been read");
                        }
                    }
                    u.detail.set_text(&s.details);
                    let now = glib::DateTime::now_local()
                        .and_then(|d| d.format("%x %X"))
                        .map(|s| s.to_string())
                        .unwrap_or_else(|_| "just now".into());
                    u.refreshed.set_text(&format!("Last checked {now}"));
                    u.ready_summary.set_text(&format!(
                        "{} is {}. Status checked {now}.",
                        kind.label(),
                        if s.active { "enabled" } else { "disabled" }
                    ));
                    u.snapshot.replace(Some(s));
                    u.render_rules();
                    if notify && mutation {
                        u.toast(success);
                    }
                }
                Err(e) => {
                    u.snapshot.replace(None);
                    u.incoming
                        .set_subtitle("Current policy could not be confirmed");
                    u.outgoing
                        .set_subtitle("Current policy could not be confirmed");
                    u.status.set_title("Firewall Status Is Unavailable");
                    u.status.set_icon_name(Some("dialog-warning-symbolic"));
                    u.status.set_description(Some("Check that the selected firewall is installed and running, then refresh to try again."));
                    u.banner.set_title(if mutation {
                        "The change could not be confirmed"
                    } else {
                        "Could not read the firewall"
                    });
                    u.banner.set_revealed(true);
                    u.details_row.set_title("Connection Details");
                    u.details_row
                        .set_subtitle("Error information and recovery guidance");
                    u.detail.set_text(&format!("{e}\n\n{}",if mutation{"The change may have been applied before the connection failed. Refresh before trying again."}else{"No changes were requested. Check administrator authorization and backend availability, then refresh."}));
                    u.count_row
                        .set_subtitle("Current configuration could not be confirmed");
                    u.ready_summary.set_text("Live status is unavailable. Open the dashboard for details and try refreshing.");
                    u.render_rules();
                    u.toast(if mutation {
                        "Change not confirmed. Open Details for recovery guidance."
                    } else {
                        "Could not refresh the firewall. Open Details to learn more."
                    });
                }
            }
            u.update_controls();
            glib::ControlFlow::Break
        });
    }
    fn render_rules(self: &Rc<Self>) {
        while let Some(child) = self.rules.first_child() {
            self.rules.remove(&child);
        }
        let query = self.search.text().trim().to_lowercase();
        let snapshot = self.snapshot.borrow();
        self.rule_group.set_description(Some(match self.kind.get() {
            Kind::Firewalld => "All zones · temporary firewalld configuration",
            Kind::Ufw => "Rules are saved by UFW",
            Kind::Mock => "Preview only · no host firewall changes",
        }));
        if let Some(s) = snapshot.as_ref() {
            let mut sorted: Vec<_> = s
                .rules
                .iter()
                .filter(|r| r.description.to_lowercase().contains(&query))
                .collect();
            match self.sort.selected() {
                1 => sorted.sort_by_key(|r| r.description.to_lowercase()),
                2 => sorted.sort_by_key(|r| r.protocol.clone()),
                3 => sorted.sort_by_key(|r| {
                    r.port
                        .as_ref()
                        .and_then(|p| p.split('-').next())
                        .and_then(|p| p.parse::<u16>().ok())
                }),
                _ => {}
            }
            for r in sorted {
                let subtitle = if self.kind.get() == Kind::Firewalld {
                    let parts: Vec<_> = r.id.splitn(3, '|').collect();
                    format!("Runtime · {}", parts.get(1).copied().unwrap_or("zone"))
                } else {
                    format!("{} · Rule {}", s.backend, r.id)
                };
                let row = adw::ActionRow::new();
                row.set_use_markup(false);
                row.set_title(&r.description);
                row.set_subtitle(&subtitle);
                row.set_title_lines(3);
                row.set_subtitle_lines(1);
                row.add_prefix(&gtk::Image::from_icon_name(if r.protocol.is_some() {
                    "network-transmit-receive-symbolic"
                } else {
                    "view-list-symbolic"
                }));
                let delete = icon_button("user-trash-symbolic", "Delete Rule");
                delete.add_css_class("flat");
                delete.set_valign(gtk::Align::Center);
                delete.update_property(&[gtk::accessible::Property::Description(&r.description)]);
                row.add_suffix(&delete);
                let u = self.clone();
                let request = Request::Delete {
                    id: r.id.clone(),
                    expected: s.revision.clone(),
                };
                let description = r.description.clone();
                delete.connect_clicked(move |_|{confirm(&u,"Delete Rule?",&format!("{description}\n\nRemoving this rule may expose a service or interrupt connections. Make sure you have a recovery path."),request.clone(),None);});
                self.rules.append(&row);
            }
        }
        let populated = self.rules.first_child().is_some();
        self.rule_stack
            .set_visible_child_name(if populated { "list" } else { "empty" });
        self.clear_search.set_visible(!query.is_empty());
        if !populated {
            let (title, description, icon) = if snapshot.is_none() {
                (
                    "Your Rules Will Appear Here",
                    "Refresh the firewall to read its current configuration.",
                    "view-list-symbolic",
                )
            } else if !query.is_empty() {
                (
                    "No Matching Rules",
                    "Try a different port, protocol, or address.",
                    "system-search-symbolic",
                )
            } else if let Some(s) = snapshot.as_ref() {
                if self.kind.get() == Kind::Ufw && !s.active {
                    ("Stored Rules Are Not Listed","UFW lists its rules when the firewall is enabled. Your stored configuration is preserved.","security-low-symbolic")
                } else {
                    ("No Rules to Show","Your firewall’s default policies still apply. Add a rule to allow or restrict specific traffic.","view-list-symbolic")
                }
            } else {
                (
                    "Your Rules Will Appear Here",
                    "Refresh the firewall to read its current configuration.",
                    "view-list-symbolic",
                )
            };
            self.empty.set_title(title);
            self.empty.set_description(Some(description));
            self.empty.set_icon_name(Some(icon));
        }
    }
}
fn focus_named(root: &gtk::Widget, name: &str) -> bool {
    if root.widget_name() == name {
        return root.grab_focus();
    }
    let mut child = root.first_child();
    while let Some(w) = child {
        if focus_named(&w, name) {
            return true;
        }
        child = w.next_sibling();
    }
    false
}
fn operation_success(request: &Request) -> &'static str {
    match request {
        Request::Status {} => "Firewall status refreshed",
        Request::SetEnabled { enabled: true, .. } => "Firewall enabled",
        Request::SetEnabled { enabled: false, .. } => "Firewall disabled",
        Request::Add { .. } => "Rule added",
        Request::Delete { .. } => "Rule deleted",
        Request::Policy { .. } => "Default policy updated",
        Request::Logging { .. } => "Logging level updated",
        Request::SaveRuntime { .. } => "Runtime configuration saved permanently",
        Request::Reload { .. } => "Permanent configuration reloaded",
        Request::Service { add: true, .. } => "Service allowed",
        Request::Service { add: false, .. } => "Service removed",
        Request::Profile { .. } => "Application profile added",
        Request::ZoneBinding { .. } => "Zone membership updated",
    }
}

// Composed native widgets avoid nested scrollers inside the dashboard's scroller.
struct StatusSummary {
    content: gtk::Box,
    icon: gtk::Image,
    title: gtk::Label,
    description: gtk::Label,
}
impl StatusSummary {
    fn new(title: &str, description: &str, icon: &str, compact: bool) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(if compact { 24 } else { 12 });
        content.set_margin_bottom(if compact { 24 } else { 6 });
        let image = gtk::Image::from_icon_name(icon);
        image.set_pixel_size(if compact { 48 } else { 64 });
        image.set_halign(if compact {
            gtk::Align::Center
        } else {
            gtk::Align::Start
        });
        image.set_valign(gtk::Align::Center);
        image.add_css_class("dim-label");
        let heading = gtk::Label::new(Some(title));
        heading.set_wrap(true);
        heading.set_xalign(if compact { 0.5 } else { 0.0 });
        heading.set_justify(if compact {
            gtk::Justification::Center
        } else {
            gtk::Justification::Left
        });
        heading.add_css_class(if compact { "title-3" } else { "title-2" });
        let detail = gtk::Label::new(Some(description));
        detail.set_wrap(true);
        detail.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        detail.set_xalign(if compact { 0.5 } else { 0.0 });
        detail.set_justify(if compact {
            gtk::Justification::Center
        } else {
            gtk::Justification::Left
        });
        detail.add_css_class("dim-label");
        if compact {
            content.append(&image);
            content.append(&heading);
            content.append(&detail);
        } else {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
            let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
            text.set_hexpand(true);
            text.append(&heading);
            text.append(&detail);
            row.append(&image);
            row.append(&text);
            content.append(&row);
        }
        Self {
            content,
            icon: image,
            title: heading,
            description: detail,
        }
    }
    fn set_title(&self, title: &str) {
        self.title.set_text(title);
    }
    fn set_description(&self, text: Option<&str>) {
        self.description.set_text(text.unwrap_or(""));
    }
    fn set_icon_name(&self, icon: Option<&str>) {
        self.icon.set_icon_name(icon);
    }
}

#[cfg(test)]
mod tests;
