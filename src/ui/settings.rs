use super::rules::{form, validation_error};
use super::*;
pub(super) fn apply_theme(ui: &Ui) {
    let theme = ui
        .settings
        .as_ref()
        .map(|s| s.string("theme").to_string())
        .unwrap_or_default();
    adw::StyleManager::default().set_color_scheme(match theme.as_str() {
        "light" => adw::ColorScheme::ForceLight,
        "dark" => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    });
}
pub(super) fn preferences(ui: &Rc<Ui>) {
    let dialog = adw::PreferencesDialog::builder()
        .title("Preferences")
        .build();
    dialog.set_widget_name("preferences");
    let general = adw::PreferencesPage::builder()
        .title("General")
        .name("general")
        .icon_name("preferences-system-symbolic")
        .build();
    let appearance = adw::PreferencesGroup::builder().title("Appearance").build();
    let theme = adw::ComboRow::builder()
        .title("Style")
        .model(&gtk::StringList::new(&["System", "Light", "Dark"]))
        .build();
    theme.set_widget_name("style-choice");
    theme.set_selected(
        ui.settings
            .as_ref()
            .map(|s| match s.string("theme").as_str() {
                "light" => 1,
                "dark" => 2,
                _ => 0,
            })
            .unwrap_or(0),
    );
    let u = ui.clone();
    theme.connect_selected_notify(move |r| {
        u.save_string("theme", ["system", "light", "dark"][r.selected() as usize]);
        adw::StyleManager::default().set_color_scheme(match r.selected() {
            1 => adw::ColorScheme::ForceLight,
            2 => adw::ColorScheme::ForceDark,
            _ => adw::ColorScheme::Default,
        });
    });
    appearance.add(&theme);
    let animations = adw::SwitchRow::builder()
        .title("Page Transitions")
        .subtitle("Always reduced when desktop animations are disabled")
        .active(ui.animations.get())
        .build();
    animations.set_widget_name("animation-choice");
    let u = ui.clone();
    animations.connect_active_notify(move |r| {
        u.animations.set(r.is_active());
        u.save_bool("animations", r.is_active());
        u.update_motion();
    });
    appearance.add(&animations);
    general.add(&appearance);
    let connection = adw::PreferencesGroup::builder()
        .title("Firewall Connection")
        .description(
            "Select an existing backend. This does not enable, disable, or switch system services.",
        )
        .build();
    let backend = adw::ComboRow::builder()
        .title("Preferred Firewall")
        .model(&gtk::StringList::new(&["UFW", "firewalld"]))
        .selected(ui.backend_picker.selected())
        .build();
    backend.set_subtitle(if ui.kind.get() == Kind::Mock {
        "Mock mode · host firewall access is disabled"
    } else {
        "Refresh after selecting a different firewall"
    });
    ui.track_selector(&backend);
    let u = ui.clone();
    backend.connect_selected_notify(move |r| u.backend_picker.set_selected(r.selected()));
    let weak = backend.downgrade();
    ui.backend_picker.connect_selected_notify(move |r| {
        if let Some(row) = weak.upgrade() {
            row.set_selected(r.selected());
        }
    });
    connection.add(&backend);
    settings_action(
        &connection,
        "Introduction",
        "Revisit the guide to rules, ports, and zones",
        "help-about-symbolic",
        {
            let u = ui.clone();
            let weak = dialog.downgrade();
            move || {
                if let Some(d) = weak.upgrade() {
                    d.close();
                }
                u.navigate("welcome");
            }
        },
    );
    general.add(&connection);
    dialog.add(&general);
    let firewall = adw::PreferencesPage::builder()
        .title("Firewall")
        .name("firewall")
        .icon_name("security-high-symbolic")
        .build();
    let group = firewall_group(ui);
    firewall.add(&group);
    dialog.add(&firewall);
    let advanced = adw::PreferencesPage::builder()
        .title("Advanced")
        .name("advanced")
        .icon_name("applications-engineering-symbolic")
        .build();
    let diagnostics_group = adw::PreferencesGroup::builder()
        .title("Troubleshooting")
        .build();
    settings_action(
        &diagnostics_group,
        "Diagnostic Report",
        "View, copy, or save the last confirmed configuration",
        "text-x-generic-symbolic",
        {
            let u = ui.clone();
            move || diagnostics(&u)
        },
    );
    settings_action(
        &diagnostics_group,
        "Refresh Firewall",
        "Read the current configuration without changing it",
        "view-refresh-symbolic",
        {
            let u = ui.clone();
            let weak = dialog.downgrade();
            move || {
                if let Some(d) = weak.upgrade() {
                    d.close();
                }
                u.navigate("dashboard");
                u.run(Request::Status {});
            }
        },
    );
    advanced.add(&diagnostics_group);
    dialog.add(&advanced);
    dialog.present(Some(&ui.window));
    let current = Rc::new(RefCell::new(group));
    let weak = firewall.downgrade();
    let u = ui.clone();
    ui.backend_picker.connect_selected_notify(move |_| {
        if let Some(page) = weak.upgrade() {
            page.remove(&*current.borrow());
            let group = firewall_group(&u);
            page.add(&group);
            current.replace(group);
        }
    });
}
fn firewall_group(ui: &Rc<Ui>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(ui.kind.get().label())
        .description(
            "Refresh the dashboard before changing configuration. Every change requires review.",
        )
        .build();
    match ui.kind.get() {
        Kind::Ufw => {
            for (title, description, kind) in [
                (
                    "Incoming Policy",
                    "Default action for connections reaching this computer",
                    Choice::Incoming,
                ),
                (
                    "Outgoing Policy",
                    "Default action for connections leaving this computer",
                    Choice::Outgoing,
                ),
                (
                    "Logging Level",
                    "Control how much firewall activity is logged",
                    Choice::Logging,
                ),
            ] {
                mutation_action(
                    ui,
                    &group,
                    title,
                    description,
                    "preferences-system-symbolic",
                    {
                        let u = ui.clone();
                        move || choice_dialog(&u, kind)
                    },
                );
            }
            mutation_action(
                ui,
                &group,
                "Allow Application Profile",
                "Use an application definition already installed on your system",
                "application-x-executable-symbolic",
                {
                    let u = ui.clone();
                    move || named_dialog(&u, false)
                },
            );
        }
        Kind::Firewalld => {
            mutation_action(
                ui,
                &group,
                "Allow Service",
                "Open a service in the default zone at runtime",
                "list-add-symbolic",
                {
                    let u = ui.clone();
                    move || named_dialog(&u, true)
                },
            );
            mutation_action(
                ui,
                &group,
                "Zone Membership",
                "Add or remove an interface or source network",
                "network-workgroup-symbolic",
                {
                    let u = ui.clone();
                    move || zone_dialog(&u)
                },
            );
            mutation_action(
                ui,
                &group,
                "Save Runtime Configuration",
                "Make all current runtime settings permanent",
                "document-save-symbolic",
                {
                    let u = ui.clone();
                    move || {
                        let s = u.snapshot.borrow().clone();
                        if let Some(s) = s {
                            confirm(&u,"Save Configuration?","This saves the entire firewalld runtime configuration, including changes made by other applications.",Request::SaveRuntime{expected:s.revision},None);
                        }
                    }
                },
            );
            mutation_action(
                ui,
                &group,
                "Reload Permanent Configuration",
                "Discard runtime-only changes and load saved settings",
                "view-refresh-symbolic",
                {
                    let u = ui.clone();
                    move || {
                        let s = u.snapshot.borrow().clone();
                        if let Some(s) = s {
                            confirm(&u,"Reload Firewall?","Runtime-only changes may be discarded. Existing connections and remote access may be interrupted.",Request::Reload{expected:s.revision},None);
                        }
                    }
                },
            );
        }
        Kind::Mock => {
            let row=adw::ActionRow::builder().title("Preview Mode").subtitle("Use the dashboard to try simulated rule changes. Backend-specific settings are available with a real firewall connection.").build();
            group.add(&row);
        }
    }
    group
}

fn settings_action(
    group: &adw::PreferencesGroup,
    title: &str,
    subtitle: &str,
    icon: &str,
    action: impl Fn() + 'static,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .activatable(true)
        .build();
    row.add_prefix(&gtk::Image::from_icon_name(icon));
    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    row.connect_activated(move |_| action());
    group.add(&row);
    row
}
fn mutation_action(
    ui: &Rc<Ui>,
    group: &adw::PreferencesGroup,
    title: &str,
    subtitle: &str,
    icon: &str,
    action: impl Fn() + 'static,
) {
    let row = settings_action(group, title, subtitle, icon, action);
    ui.track_mutation(&row);
}
#[derive(Clone, Copy)]
enum Choice {
    Incoming,
    Outgoing,
    Logging,
}
fn choice_dialog(ui: &Rc<Ui>, choice: Choice) {
    let (title, items) = match choice {
        Choice::Incoming => ("Incoming Policy", vec!["Allow", "Deny", "Reject"]),
        Choice::Outgoing => ("Outgoing Policy", vec!["Allow", "Deny", "Reject"]),
        Choice::Logging => (
            "Logging Level",
            vec!["Off", "Low", "Medium", "High", "Full"],
        ),
    };
    let (d, body, review, _) = form(ui, title);
    d.set_content_height(300);
    let current = current_choice(ui, choice).and_then(|value| {
        items
            .iter()
            .position(|item| item.eq_ignore_ascii_case(&value))
    });
    let description = if let Some(index) = current {
        format!(
            "Last read from the firewall: {}. Choose a different value, then review the change.",
            items[index]
        )
    } else {
        "Choose a proposed value. The current configuration is available in the dashboard’s Configuration Details.".into()
    };
    let group = adw::PreferencesGroup::builder()
        .description(description)
        .build();
    let mut choices = vec!["Choose…"];
    choices.extend(items.iter().copied());
    let selection = adw::ComboRow::builder()
        .title(title)
        .model(&gtk::StringList::new(&choices))
        .selected(current.map(|i| i as u32 + 1).unwrap_or(0))
        .build();
    review.set_sensitive(false);
    let r = review.clone();
    selection.connect_selected_notify(move |row| {
        r.set_sensitive(row.selected() > 0 && Some(row.selected() as usize - 1) != current)
    });
    group.add(&selection);
    body.append(&group);
    let u = ui.clone();
    let weak = d.downgrade();
    d.set_focus(Some(&selection));
    review.connect_clicked(move |_|{let snapshot=u.snapshot.borrow().clone();if let Some(s)=snapshot{let Some(index)=(selection.selected()as usize).checked_sub(1)else{return};let(request,heading,message)=match choice{
        Choice::Logging=>(Request::Logging{level:["off","low","medium","high","full"][index].into(),expected:s.revision},"Change Logging Level?",format!("Set firewall logging to {}. Higher levels may generate substantial log traffic.",items[index])),
        _=>{let direction=if matches!(choice,Choice::Incoming){"incoming"}else{"outgoing"};(Request::Policy{direction:direction.into(),policy:["allow","deny","reject"][index].into(),expected:s.revision},"Change Default Policy?",format!("Set the default {direction} policy to {}. This may interrupt remote connections. Ensure explicit rules preserve your remote access and that you have a recovery path.",items[index]))}
    };let weak=weak.clone();confirm(&u,heading,&message,request,Some(Box::new(move ||{if let Some(d)=weak.upgrade(){d.close();}})));}});
    d.present(Some(&ui.window));
}
fn current_choice(ui: &Ui, choice: Choice) -> Option<String> {
    let state = ui.snapshot.borrow();
    let details = &state.as_ref()?.details;
    match choice {
        Choice::Logging => {
            let line = details.lines().find_map(|l| l.strip_prefix("Logging: "))?;
            if line == "off" {
                Some("off".into())
            } else {
                line.split_once('(')
                    .and_then(|(_, s)| s.split_once(')'))
                    .map(|(level, _)| level.to_string())
            }
        }
        _ => {
            let direction = if matches!(choice, Choice::Incoming) {
                "(incoming)"
            } else {
                "(outgoing)"
            };
            let line = details.lines().find_map(|l| l.strip_prefix("Default: "))?;
            line.split(',')
                .find(|s| s.contains(direction))
                .and_then(|s| s.split_whitespace().next())
                .map(str::to_string)
        }
    }
}
fn named_dialog(ui: &Rc<Ui>, service: bool) {
    let (d, body, review, error) = form(
        ui,
        if service {
            "Allow Service"
        } else {
            "Allow Application"
        },
    );
    d.set_content_height(320);
    let group=adw::PreferencesGroup::builder().description(if service{"Use an installed firewalld service name, such as ssh or https. Applies to the default zone at runtime."}else{"Use an installed UFW application profile. Available profiles are listed in Configuration Details on the dashboard."}).build();
    let entry = adw::EntryRow::builder()
        .title(if service {
            "Service Name"
        } else {
            "Application Profile"
        })
        .build();
    entry.set_activates_default(true);
    group.add(&entry);
    body.append(&group);
    review.set_sensitive(false);
    let r = review.clone();
    let e = error.clone();
    entry.connect_changed(move |entry| {
        r.set_sensitive(!entry.text().trim().is_empty());
        e.set_visible(false);
    });
    let u = ui.clone();
    let weak = d.downgrade();
    d.set_focus(Some(&entry));
    review.connect_clicked(move |_|{let name=entry.text().trim().to_string();if name.is_empty()||name.starts_with('-')||name.contains(['\n','\r']){validation_error(&error,"Enter an installed service or profile name.");return;}
        let snapshot=u.snapshot.borrow().clone();if let Some(s)=snapshot{let request=if service{Request::Service{name:name.clone(),add:true,expected:s.revision}}else{Request::Profile{name:name.clone(),action:"allow".into(),expected:s.revision}};let weak=weak.clone();confirm(&u,if service{"Allow This Service?"}else{"Allow This Application?"},&format!("Allow {name}. This permits the network traffic defined by the installed {}. Review its ports before applying.",if service{"service"}else{"profile"}),request,Some(Box::new(move ||{if let Some(d)=weak.upgrade(){d.close();}})));}
    });
    d.present(Some(&ui.window));
}
fn zone_dialog(ui: &Rc<Ui>) {
    let (d, body, review, error) = form(ui, "Zone Membership");
    d.set_content_height(400);
    let group=adw::PreferencesGroup::builder().description("Runtime changes only. Changing a connection’s trust zone can expose services or interrupt remote access.").build();
    let zone = adw::EntryRow::builder()
        .title("Zone")
        .text(
            ui.snapshot
                .borrow()
                .as_ref()
                .map(|s| s.zone.as_str())
                .unwrap_or(""),
        )
        .build();
    let kind = adw::ComboRow::builder()
        .title("Change")
        .model(&gtk::StringList::new(&[
            "Add Interface",
            "Remove Interface",
            "Add Source Network",
            "Remove Source Network",
        ]))
        .build();
    let value = adw::EntryRow::builder().title("Interface Name").build();
    zone.set_activates_default(true);
    value.set_activates_default(true);
    group.add(&zone);
    group.add(&kind);
    group.add(&value);
    body.append(&group);
    let v = value.clone();
    kind.connect_selected_notify(move |k| {
        v.set_title(if k.selected() >= 2 {
            "Source IP or Network"
        } else {
            "Interface Name"
        })
    });
    review.set_sensitive(false);
    let r = review.clone();
    value.connect_changed(move |v| r.set_sensitive(!v.text().trim().is_empty()));
    let u = ui.clone();
    let weak = d.downgrade();
    d.set_focus(Some(&value));
    review.connect_clicked(move |_|{
        let zone=zone.text().trim().to_string();let value=value.text().trim().to_string();let source=kind.selected()>=2;
        if zone.is_empty(){validation_error(&error,"Enter a zone name.");return;}
        if source{let spec=RuleSpec{port:"1".into(),protocol:"tcp".into(),action:"allow".into(),direction:"in".into(),source:value.clone(),destination:"any".into()};if value=="any"||spec.validate().is_err(){validation_error(&error,"Enter an IPv4 or IPv6 address or CIDR network.");return;}}else if value.is_empty()||value.len()>15||!value.chars().all(|c|c.is_ascii_alphanumeric()||"_.-:".contains(c)){validation_error(&error,"Enter a valid network interface name.");return;}
        let s=u.snapshot.borrow().clone();if let Some(s)=s{let weak=weak.clone();let add=kind.selected().is_multiple_of(2);confirm(&u,"Change Zone Membership?",&format!("{} {value} {} zone {zone}. Keep local recovery access available before changing the connection’s trust settings.",if add{"Add"}else{"Remove"},if add{"to"}else{"from"}),Request::ZoneBinding{zone,value,source,add,expected:s.revision},Some(Box::new(move ||{if let Some(d)=weak.upgrade(){d.close();}})));}
    });
    d.present(Some(&ui.window));
}
pub(super) fn diagnostics(ui: &Rc<Ui>) {
    let report = format!(
        "Ward {}\nSelected: {}\n{}",
        env!("CARGO_PKG_VERSION"),
        ui.kind.get().label(),
        ui.snapshot
            .borrow()
            .as_ref()
            .map(|s| serde_json::to_string_pretty(s).unwrap_or_default())
            .unwrap_or_else(|| format!(
                "No confirmed status. Refresh the firewall.\n{}",
                ui.detail.text()
            ))
    );
    let (d, text, header, body) = text_dialog("Diagnostic Report", 620);
    d.set_widget_name("diagnostic-report");
    text.buffer().set_text(&report);
    let note = gtk::Label::new(Some(
        "This report can contain IP addresses and rules. Review it before sharing.",
    ));
    note.set_wrap(true);
    note.add_css_class("caption");
    note.add_css_class("dim-label");
    margins(&note, 12);
    body.prepend(&note);
    let copy = icon_button("edit-copy-symbolic", "Copy Report");
    let u = ui.clone();
    let data = report.clone();
    copy.connect_clicked(move |_| {
        u.window.clipboard().set_text(&data);
        u.toast("Diagnostic report copied");
    });
    header.pack_end(&copy);
    let save = icon_button("document-save-symbolic", "Save Report");
    let u = ui.clone();
    save.connect_clicked(move |_| {
        let file_dialog = gtk::FileDialog::builder()
            .title("Save Diagnostic Report")
            .initial_name("ward-diagnostics.txt")
            .build();
        let u = u.clone();
        let data = report.clone();
        glib::spawn_future_local(async move {
            match file_dialog.save_future(Some(&u.window)).await {
                Ok(file) => match file
                    .replace_contents_future(
                        data.into_bytes(),
                        None,
                        false,
                        gio::FileCreateFlags::PRIVATE,
                    )
                    .await
                {
                    Ok(_) => u.toast("Diagnostic report saved"),
                    Err((_, e)) => u.toast(&format!("Could not save the report: {e}")),
                },
                Err(e) if e.matches(gtk::DialogError::Dismissed) => {}
                Err(e) => u.toast(&format!("Could not open the save dialog: {e}")),
            }
        });
    });
    header.pack_end(&save);
    d.present(Some(&ui.window));
}
pub(super) fn text_dialog(
    title: &str,
    width: i32,
) -> (adw::Dialog, gtk::TextView, adw::HeaderBar, gtk::Box) {
    let d = adw::Dialog::builder()
        .title(title)
        .content_width(width)
        .content_height(520)
        .build();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar.add_top_bar(&header);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
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
    scroll.set_vexpand(true);
    scroll.set_child(Some(&text));
    body.append(&scroll);
    toolbar.set_content(Some(&body));
    d.set_child(Some(&toolbar));
    (d, text, header, body)
}
pub(super) fn show_details(ui: &Rc<Ui>) {
    let (d, text, _, _) = text_dialog("Firewall Details", 620);
    text.buffer().set_text(&ui.detail.text());
    d.present(Some(&ui.window));
}
pub(super) fn about(ui: &Rc<Ui>) {
    let about = adw::AboutDialog::builder()
        .application_name("Ward")
        .application_icon(ID)
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("Ward contributors")
        .license_type(gtk::License::Gpl30)
        .comments("A simpler way to manage your Linux firewall.")
        .build();
    about.present(Some(&ui.window));
}
pub(super) fn shortcuts(ui: &Rc<Ui>) {
    let d = adw::PreferencesDialog::builder()
        .title("Keyboard Shortcuts")
        .build();
    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::new();
    for (title, key) in [
        ("Refresh Firewall", "Ctrl+R / F5"),
        ("Add Rule", "Ctrl+N"),
        ("Search Rules", "Ctrl+F"),
        ("Preferences", "Ctrl+,"),
        ("Close Dialog", "Esc"),
    ] {
        let row = adw::ActionRow::builder().title(title).build();
        row.add_suffix(&gtk::Label::new(Some(key)));
        group.add(&row);
    }
    page.add(&group);
    d.add(&page);
    d.present(Some(&ui.window));
}
