use super::*;

pub(super) fn heading(title: &str, description: &str) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let title = gtk::Label::new(Some(title));
    title.set_xalign(0.0);
    title.add_css_class("title-1");
    let description = gtk::Label::new(Some(description));
    description.set_xalign(0.0);
    description.set_wrap(true);
    description.add_css_class("dim-label");
    container.append(&title);
    container.append(&description);
    container
}

pub(super) fn action_card(title: &str, description: &str, icon: &str) -> (gtk::Button, gtk::Label) {
    let button = gtk::Button::new();
    button.add_css_class("card");
    button.set_hexpand(true);
    button.set_valign(gtk::Align::Fill);
    button.update_property(&[
        gtk::accessible::Property::Label(title),
        gtk::accessible::Property::Description(description),
    ]);
    let body = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    margins(&body, 12);
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(28);
    image.add_css_class("accent");
    body.append(&image);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
    text.set_hexpand(true);
    let label = gtk::Label::new(Some(title));
    label.add_css_class("heading");
    label.set_xalign(0.0);
    label.set_wrap(true);
    let detail = gtk::Label::new(Some(description));
    detail.set_xalign(0.0);
    detail.set_wrap(true);
    detail.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    detail.add_css_class("dim-label");
    text.append(&label);
    text.append(&detail);
    body.append(&text);
    let arrow = gtk::Image::from_icon_name("go-next-symbolic");
    arrow.add_css_class("dim-label");
    body.append(&arrow);
    button.set_child(Some(&body));
    (button, detail)
}

pub(super) fn reported_policy(details: &str, direction: &str) -> String {
    let value = details
        .lines()
        .find_map(|l| l.strip_prefix("Default: "))
        .and_then(|line| {
            line.split(',')
                .find(|part| part.contains(&format!("({direction})")))
        })
        .and_then(|part| part.split_whitespace().next());
    match value {
        Some("allow") => "Allowed unless a rule says otherwise".into(),
        Some("deny") => "Blocked unless a rule allows it".into(),
        Some("reject") => "Rejected unless a rule allows it".into(),
        _ => "Not reported by UFW; see configuration details".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explain_only_reported_policies() {
        let raw = "Status: active\nDefault: deny (incoming), allow (outgoing), disabled (routed)\n";
        assert!(reported_policy(raw, "incoming").starts_with("Blocked"));
        assert!(reported_policy(raw, "outgoing").starts_with("Allowed"));
        assert!(reported_policy("Status: inactive", "incoming").starts_with("Not reported"));
    }
}
