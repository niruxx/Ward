use super::*;
// Keep the editor underneath review so Cancel returns to the user's input.
pub(super) fn confirm(
    ui: &Rc<Ui>,
    title: &str,
    body: &str,
    request: Request,
    accepted: Option<Box<dyn Fn()>>,
) {
    confirm_as(ui, title, body, request, accepted, None);
}
fn confirm_as(
    ui: &Rc<Ui>,
    title: &str,
    body: &str,
    request: Request,
    accepted: Option<Box<dyn Fn()>>,
    action_label: Option<&str>,
) {
    if ui.busy.get() {
        ui.toast("Wait for the current firewall operation to finish.");
        return;
    }
    if ui.snapshot.borrow().is_none() {
        ui.toast("Refresh the firewall before making a change.");
        return;
    }
    let (label, destructive) = match &request {
        Request::Delete { .. } => ("Delete Rule", true),
        Request::SetEnabled { enabled: true, .. } => ("Enable Firewall", false),
        Request::SetEnabled { enabled: false, .. } => ("Disable Firewall", true),
        Request::Add { rule, .. } => ("Add Rule", rule.action != "allow"),
        Request::Reload { .. } => ("Reload", true),
        Request::SaveRuntime { .. } => ("Save Permanently", false),
        Request::Policy { .. } => ("Change Policy", true),
        Request::Logging { .. } => ("Change Logging", false),
        Request::Service { .. } => ("Allow Service", false),
        Request::Profile { .. } => ("Allow Application", false),
        Request::ZoneBinding { .. } => ("Change Membership", true),
        Request::Status {} => ("Refresh", false),
    };
    let dialog = adw::AlertDialog::builder()
        .heading(title)
        .body(body)
        .build();
    dialog.set_widget_name("change-review");
    dialog.add_responses(&[
        ("cancel", "Cancel"),
        ("apply", action_label.unwrap_or(label)),
    ]);
    dialog.set_response_appearance(
        "apply",
        if destructive {
            adw::ResponseAppearance::Destructive
        } else {
            adw::ResponseAppearance::Suggested
        },
    );
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let u = ui.clone();
    dialog.connect_response(None, move |_, response| {
        if response == "apply" {
            if let Some(callback) = &accepted {
                callback();
            }
            u.run(request.clone());
        }
    });
    dialog.present(Some(&ui.window));
}
pub(super) fn form(_ui: &Rc<Ui>, title: &str) -> (adw::Dialog, gtk::Box, gtk::Button, gtk::Label) {
    let dialog = adw::Dialog::builder()
        .title(title)
        .content_width(480)
        .content_height(560)
        .build();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_show_start_title_buttons(false);
    header.set_show_end_title_buttons(false);
    let cancel = gtk::Button::with_label("Cancel");
    let weak = dialog.downgrade();
    cancel.connect_clicked(move |_| {
        if let Some(d) = weak.upgrade() {
            d.close();
        }
    });
    header.pack_start(&cancel);
    let review = gtk::Button::with_label("Review");
    review.add_css_class("suggested-action");
    review.set_widget_name("form-review");
    header.pack_end(&review);
    toolbar.add_top_bar(&header);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 18);
    margins(&body, 18);
    let error = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .accessible_role(gtk::AccessibleRole::Alert)
        .build();
    error.set_widget_name("form-error");
    error.add_css_class("error");
    error.set_visible(false);
    let container = gtk::Box::new(gtk::Orientation::Vertical, 12);
    container.append(&body);
    container.append(&error);
    error.set_margin_start(18);
    error.set_margin_end(18);
    error.set_margin_bottom(18);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&container));
    toolbar.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar));
    dialog.set_default_widget(Some(&review));
    (dialog, body, review, error)
}
pub(super) fn validation_error(label: &gtk::Label, text: &str) {
    label.set_text(text);
    label.set_visible(true);
}
pub(super) fn rule_dialog(ui: &Rc<Ui>) {
    if ui.window.visible_dialog().is_some() {
        return;
    }
    if ui.busy.get() || ui.snapshot.borrow().is_none() {
        ui.toast("Refresh the firewall before adding a rule.");
        return;
    }
    let (dialog, body, review, error) = form(ui, "Add Rule");
    dialog.set_widget_name("rule-editor");
    let firewalld = ui.kind.get() == Kind::Firewalld;
    let context=adw::PreferencesGroup::builder().title("Traffic").description(if firewalld{"Create an incoming rule in the default firewalld zone. Blocking and address restrictions use rich rules. Changes are runtime-only until saved."}else{"Your firewall will save this rule. Port ranges use a dash, for example 8000–8010 (enter 8000-8010)."}).build();
    let port = adw::EntryRow::builder().title("Port or Range").build();
    port.set_widget_name("rule-port");
    context.add(&port);
    let protocol = adw::ComboRow::builder()
        .title("Protocol")
        .model(&gtk::StringList::new(&["TCP", "UDP"]))
        .build();
    context.add(&protocol);
    let action = adw::ComboRow::builder()
        .title("Action")
        .model(&gtk::StringList::new(if firewalld {
            &["Allow", "Block", "Reject"]
        } else {
            &["Allow", "Block", "Reject", "Limit"]
        }))
        .build();
    let direction = adw::ComboRow::builder()
        .title("Direction")
        .model(&gtk::StringList::new(&["Incoming", "Outgoing"]))
        .build();
    context.add(&action);
    action.set_subtitle("Let matching connections through");
    action.connect_selected_notify(move |a| {
        a.set_subtitle(match a.selected() {
            0 => "Let matching connections through",
            1 => "Silently drop matching traffic",
            2 => "Refuse the connection and notify its sender",
            _ => "UFW TCP connection rate limiting",
        })
    });
    if !firewalld {
        context.add(&direction);
    }
    body.append(&context);
    let addresses = adw::PreferencesGroup::builder()
        .title("Addresses")
        .description("Enter an IPv4 or IPv6 address, a CIDR network, or “any”.")
        .build();
    let source = adw::EntryRow::builder().title("Source").text("any").build();
    let destination = adw::EntryRow::builder()
        .title("Destination")
        .text("any")
        .build();
    source.set_activates_default(true);
    destination.set_activates_default(true);
    addresses.add(&source);
    addresses.add(&destination);
    body.append(&addresses);
    let notice = gtk::Label::new(Some("You will review this rule before it is applied. Adding it does not remove other existing rules."));
    notice.set_wrap(true);
    notice.add_css_class("caption");
    notice.add_css_class("dim-label");
    body.append(&notice);
    review.set_sensitive(false);
    let r = review.clone();
    let e = error.clone();
    port.connect_changed(move |p| {
        r.set_sensitive(!p.text().trim().is_empty());
        p.remove_css_class("error");
        e.set_visible(false);
    });
    port.set_activates_default(true);
    let u = ui.clone();
    let d = dialog.clone();
    dialog.set_focus(Some(&port));
    review.connect_clicked(move |_|{
        let rule=RuleSpec{port:port.text().trim().into(),protocol:if protocol.selected()==0{"tcp"}else{"udp"}.into(),action:["allow","deny","reject","limit"][action.selected()as usize].into(),direction:if direction.selected()==0{"in"}else{"out"}.into(),source:source.text().trim().into(),destination:destination.text().trim().into()};
        if let Err(e)=rule.validate(){validation_error(&error,&e);if crate::model::validate_port(&rule.port).is_err(){port.add_css_class("error");d.set_focus(Some(&port));}return;}
        if firewalld {if let Err(e)=backend::firewalld::rich_rule(&rule){validation_error(&error,&e);return;}}
        let snapshot=u.snapshot.borrow().clone();if let Some(s)=snapshot{
            let scope=if firewalld{format!("Runtime · zone {}",s.zone)}else{"Persistent configuration".into()};
            let action_name=match rule.action.as_str(){"allow"=>"Allow","deny"=>"Block","reject"=>"Reject",_=>"Rate-limit"};
            let body=format!("{} {} traffic on {}/{}\nSource: {}\nDestination: {}\n{}\n\nThis may affect remote access. Make sure you have a recovery path.",action_name,if rule.direction=="in"{"incoming"}else{"outgoing"},rule.port,rule.protocol,rule.source,rule.destination,scope);
            let weak=d.downgrade();confirm(&u,"Add This Rule?",&body,Request::Add{rule,expected:s.revision},Some(Box::new(move ||{if let Some(d)=weak.upgrade(){d.close();}})));
        }else{validation_error(&error,"Refresh the firewall before adding a rule.");}
    });
    dialog.present(Some(&ui.window));
}

/// A port exception is an ordinary incoming allow rule, not a separate
/// privileged operation. It uses the same validation and backend boundary.
pub(super) fn port_dialog(ui: &Rc<Ui>) {
    if ui.window.visible_dialog().is_some() {
        return;
    }
    if ui.busy.get() || ui.snapshot.borrow().is_none() {
        ui.toast("Read the firewall status before allowing a port.");
        return;
    }
    let firewalld = ui.kind.get() == Kind::Firewalld;
    let (dialog, body, review, error) = form(ui, "Allow Port");
    dialog.set_widget_name("port-editor");
    dialog.set_content_height(if firewalld { 380 } else { 560 });
    let group=adw::PreferencesGroup::builder().title("Let an App Receive Connections").description("A port exception is an incoming allow rule. Only open a port for a service you intend to share.").build();
    let port = adw::EntryRow::builder()
        .title("Port or Range")
        .activates_default(true)
        .build();
    port.set_widget_name("exception-port");
    group.add(&port);
    let protocol = adw::ComboRow::builder()
        .title("Protocol")
        .model(&gtk::StringList::new(&["TCP", "UDP"]))
        .build();
    protocol.set_widget_name("exception-protocol");
    group.add(&protocol);
    body.append(&group);
    let hint = gtk::Label::new(Some("For example: 443, or a range such as 8000-8010."));
    hint.set_wrap(true);
    hint.set_xalign(0.0);
    hint.add_css_class("caption");
    hint.add_css_class("dim-label");
    body.append(&hint);
    let audience = adw::PreferencesGroup::builder()
        .title("Who Can Connect?")
        .build();
    let restriction = adw::ComboRow::builder()
        .title("Source")
        .model(&gtk::StringList::new(&[
            "Any Address",
            "Specific IP or Network",
        ]))
        .build();
    let source = adw::EntryRow::builder()
        .title("Source IP or Network")
        .activates_default(true)
        .build();
    restriction.set_widget_name("exception-audience");
    source.set_widget_name("exception-source");
    source.set_visible(false);
    if !firewalld {
        audience.add(&restriction);
        audience.add(&source);
        body.append(&audience);
        let field = source.clone();
        restriction.connect_selected_notify(move |r| field.set_visible(r.selected() == 1));
    }
    let scope = gtk::Label::new(Some(if firewalld {
        "Applies to the default firewalld zone, from any address. This exception is runtime-only and can disappear after a reload."
    } else {
        "Saved by UFW. If UFW is off, the exception takes effect when you enable it."
    }));
    scope.set_xalign(0.0);
    scope.set_wrap(true);
    scope.add_css_class("caption");
    scope.add_css_class("dim-label");
    body.append(&scope);
    review.set_sensitive(false);
    let button = review.clone();
    let feedback = error.clone();
    port.connect_changed(move |p| {
        button.set_sensitive(!p.text().trim().is_empty());
        p.remove_css_class("error");
        feedback.set_visible(false);
    });
    let source_feedback = error.clone();
    source.connect_changed(move |field| {
        field.remove_css_class("error");
        source_feedback.set_visible(false);
    });
    dialog.set_focus(Some(&port));
    let u = ui.clone();
    let d = dialog.clone();
    review.connect_clicked(move |_|{
        let rule=RuleSpec{port:port.text().trim().into(),protocol:if protocol.selected()==0{"tcp"}else{"udp"}.into(),action:"allow".into(),direction:"in".into(),source:if !firewalld&&restriction.selected()==1{source.text().trim().into()}else{"any".into()},destination:"any".into()};
        if !firewalld&&restriction.selected()==1&&rule.source=="any"{validation_error(&error,"Enter a specific IP address or network, or choose Any Address.");return;}
        if let Err(message)=rule.validate(){validation_error(&error,&message);if crate::model::validate_port(&rule.port).is_err(){port.add_css_class("error");d.set_focus(Some(&port));}else{source.add_css_class("error");d.set_focus(Some(&source));}return;}
        let snapshot=u.snapshot.borrow().clone();if let Some(s)=snapshot{
            let scope=if firewalld{format!("Default zone: {}. Runtime only, until a reload or restart unless saved permanently.",s.zone)}else{"Saved by UFW. Applies when the firewall is enabled.".into()};
            let message=format!("Allow incoming {} connections on port {}.\nWho can connect: {}\n\n{}\n\nThis opens access to any service listening on that port. Make sure it is a service you intend to share.",rule.protocol.to_uppercase(),rule.port,if rule.source=="any"{"any address"}else{&rule.source},scope);
            let weak=d.downgrade();confirm_as(&u,"Allow This Port?",&message,Request::Add{rule,expected:s.revision},Some(Box::new(move ||{if let Some(d)=weak.upgrade(){d.close();}})),Some("Allow Port"));
        }else{validation_error(&error,"Read the firewall status before adding this exception.");}
    });
    dialog.present(Some(&ui.window));
}
