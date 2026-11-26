use super::*;
pub(super) fn onboarding(ui: &Rc<Ui>) {
    let pages = [
        (
            "welcome",
            "Welcome to Ward",
            "A simpler way to manage your Linux firewall.",
            ID,
            "Get Started",
        ),
        (
            "detect",
            "Connect Your Firewall",
            "Use the firewall already installed on your computer.",
            "network-workgroup-symbolic",
            "Continue",
        ),
        (
            "learn",
            "Understand Your Traffic",
            "Decide which connections can reach your computer — and which can leave it.",
            "network-transmit-receive-symbolic",
            "Continue",
        ),
        (
            "ready",
            "Ready to Open Ward",
            "Review your configuration, make a change, and stay in control.",
            "security-high-symbolic",
            "Open Ward",
        ),
    ];
    for (i, (name, title, description, icon, label)) in pages.iter().enumerate() {
        let page = adw::StatusPage::builder()
            .title(*title)
            .description(*description)
            .icon_name(*icon)
            .build();
        if i > 0 {
            page.add_css_class("compact");
        }
        let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
        let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.set_vexpand(true);
        outer.append(&page);
        let footer = gtk::Box::new(gtk::Orientation::Vertical, 8);
        margins(&footer, 12);
        match i {
            0 => {
                let note = gtk::Label::new(Some(
                    "Your existing rules stay yours. Nothing changes until you apply it.",
                ));
                note.set_wrap(true);
                note.set_justify(gtk::Justification::Center);
                note.add_css_class("dim-label");
                content.append(&note);
            }
            1 => {
                let group = adw::PreferencesGroup::new();
                let picker = adw::ActionRow::builder()
                    .title("Firewall Backend")
                    .subtitle("UFW or firewalld")
                    .build();
                picker.set_activatable_widget(Some(&ui.backend_picker));
                ui.backend_picker.set_valign(gtk::Align::Center);
                picker.add_suffix(&ui.backend_picker);
                group.add(&picker);
                let explanation = adw::ExpanderRow::builder()
                    .title("Which Firewall Should I Use?")
                    .build();
                let text=gtk::Label::new(Some("UFW offers straightforward, persistent rules. firewalld groups services and connections into trust zones, with separate runtime and permanent configuration. Choose the one your system already uses."));
                text.set_wrap(true);
                text.set_xalign(0.0);
                margins(&text, 16);
                explanation.add_row(&text);
                group.add(&explanation);
                content.append(&group);
                content.append(&ui.detection);
                content.append(&ui.check_backend);
                let checked = gtk::Label::new(None);
                checked.set_wrap(true);
                checked.set_justify(gtk::Justification::Center);
                checked.add_css_class("caption");
                ui.ready_summary
                    .bind_property("label", &checked, "label")
                    .sync_create()
                    .build();
                content.append(&checked);
                let note = gtk::Label::new(Some(
                    "Reading UFW status requires administrator authorization.",
                ));
                note.set_wrap(true);
                note.add_css_class("caption");
                note.add_css_class("dim-label");
                content.append(&note);
            }
            2 => {
                let group = adw::PreferencesGroup::new();
                for(title,subtitle,icon)in [
                    ("Incoming & Outgoing","Incoming connections reach this computer. Outgoing connections leave it.","network-transmit-receive-symbolic"),
                    ("Rules & Ports","Rules allow or restrict traffic to a particular service or network port.","view-list-symbolic"),
                    ("Services & Zones","firewalld services define ports. Zones apply different trust settings to connections.","network-workgroup-symbolic")
                ]{let row=adw::ActionRow::new();row.set_use_markup(false);row.set_title(title);row.set_subtitle(subtitle);row.set_subtitle_lines(0);row.add_prefix(&gtk::Image::from_icon_name(icon));group.add(&row);}
                content.append(&group);
            }
            _ => {
                let selected = adw::ActionRow::builder()
                    .title("Selected Firewall")
                    .subtitle(ui.kind.get().label())
                    .build();
                selected.add_prefix(&gtk::Image::from_icon_name("network-workgroup-symbolic"));
                let weak = selected.downgrade();
                let u = ui.clone();
                ui.backend_picker.connect_selected_notify(move |p| {
                    if let Some(row) = weak.upgrade() {
                        row.set_subtitle(if u.kind.get() == Kind::Mock {
                            Kind::Mock.label()
                        } else if p.selected() == 1 {
                            Kind::Firewalld.label()
                        } else {
                            Kind::Ufw.label()
                        });
                    }
                });
                let group = adw::PreferencesGroup::new();
                group.add(&selected);
                content.append(&group);
                content.append(&ui.ready_summary);
                let note=gtk::Label::new(Some("Setup does not change your firewall. Keep a recovery path available before changing remote access."));
                note.set_wrap(true);
                note.set_justify(gtk::Justification::Center);
                note.add_css_class("caption");
                note.add_css_class("dim-label");
                content.append(&note);
            }
        }
        let navigation = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        navigation.set_halign(gtk::Align::Center);
        if i > 0 {
            let back = gtk::Button::with_label("Back");
            back.add_css_class("pill");
            let u = ui.clone();
            let previous = pages[i - 1].0;
            back.connect_clicked(move |_| u.navigate(previous));
            navigation.append(&back);
        }
        let next = gtk::Button::with_label(label);
        next.add_css_class("suggested-action");
        next.add_css_class("pill");
        next.set_widget_name("onboarding-next");
        navigation.append(&next);
        footer.append(&navigation);
        let progress = gtk::Label::new(Some(&format!("{} of {}", i + 1, pages.len())));
        progress.add_css_class("caption");
        progress.add_css_class("dim-label");
        footer.append(&progress);
        outer.append(&footer);
        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(440);
        clamp.set_tightening_threshold(360);
        clamp.set_child(Some(&content));
        page.set_child(Some(&clamp));
        ui.stack.add_named(&outer, Some(name));
        let u = ui.clone();
        let destination = if i == 3 { "dashboard" } else { pages[i + 1].0 };
        next.connect_clicked(move |_| {
            if destination == "dashboard" {
                u.completed.set(true);
                u.save_bool("onboarding-complete", true);
            }
            u.navigate(destination);
            if destination == "dashboard"
                && u.kind.get() == Kind::Firewalld
                && u.snapshot.borrow().is_none()
            {
                u.run_with_feedback(Request::Status {}, false);
            }
        });
    }
}
