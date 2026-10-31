//! Licensed services: the page where the viewer chooses the streaming services they pay for,
//! what a title's page says about them (the three cases the engine works out), and the notice
//! for coming back from a service. anistream opens these in the browser and never plays them.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use anistream_core::ids::AnilistId;
use anistream_i18n::t;
use anistream_model::{
    command::{Command, LicensedCommand},
    event::Event,
    licensed::{AvailabilityCase, LicensedView, ServiceLink, TitleAvailability},
};
use gtk::glib;

use crate::{pages::first_party, widgets, window::Ctx};

/// How many services show before "Show all".
const FIRST: usize = 8;

/// The country the system is set to, from its locale: `da_DK.UTF-8` is `DK`.
pub fn system_region() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|key| std::env::var(key).ok())
        .find(|value| !value.is_empty() && value != "C" && value != "POSIX")
        .and_then(|value| {
            let territory = value.split('.').next()?.split('_').nth(1)?;
            let territory = territory.split('@').next()?.to_ascii_uppercase();
            (territory.len() == 2).then_some(territory)
        })
}

/// Ask the engine for the page, and again when it changes.
pub fn load(hub: &crate::hub::Hub) {
    hub.send(Command::Licensed(LicensedCommand::Load { system_region: system_region() }));
}

/// A Sources row's line: the services the viewer chose and the region.
pub fn line(view: Option<&LicensedView>) -> String {
    match view {
        Some(view) if !view.settings.services.is_empty() => {
            let region = view.region.clone().unwrap_or_default();
            let services = view.settings.services.join(", "); // i18n-skip
            if region.is_empty() { services } else { t::licensed_row_line(&services, &region) }
        }
        _ => t::licensed_row_none(),
    }
}

/// The Licensed services row on the Sources page.
pub fn row(ctx: &Rc<Ctx>, view: Option<&LicensedView>) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(t::licensed_title())
        .subtitle(line(view))
        .activatable(true)
        .build();
    row.set_title_lines(1);
    row.set_subtitle_lines(1);
    row.add_suffix(&widgets::chevron());
    let weak = Rc::downgrade(ctx);
    row.connect_activated(move |_| {
        if let Some(ctx) = weak.upgrade() {
            push(&ctx);
        }
    });
    row
}

fn redraw(event: &Event) -> bool {
    matches!(event, Event::Licensed(_))
}

/// Go to Sources and push the Licensed services page: Choose your services.
pub fn show(ctx: &Rc<Ctx>) {
    let _ = WidgetExt::activate_action(&ctx.window, "win.go", Some(&"providers".to_variant()));
    push(ctx);
}

/// Push the Licensed services page.
pub fn push(ctx: &Rc<Ctx>) {
    load(&ctx.hub);
    // Choosing services is not a redraw of the checks under the viewer's finger: the page draws
    // once per answer, and an answer to a tick changes only what the tick changed.
    let shown_all = Rc::new(RefCell::new(false));
    first_party::push(
        ctx,
        "licensed",
        &adw::HeaderBar::new(),
        redraw,
        move |ctx, page, column| {
            page.set_title(&t::licensed_title());
            let view = ctx.hub.known.borrow().licensed.clone();
            fill(ctx, column, view.as_ref(), &shown_all);
        },
    );
}

fn fill(
    ctx: &Rc<Ctx>,
    column: &gtk::Box,
    view: Option<&LicensedView>,
    shown_all: &Rc<RefCell<bool>>,
) {
    first_party::title_block(column, &t::licensed_title(), &t::licensed_body());
    let Some(view) = view else {
        let waiting = widgets::haze(&t::licensed_loading(), "");
        waiting.set_margin_top(18);
        column.append(&waiting);
        return;
    };

    // Region.
    let region_list = widgets::boxed_list();
    region_list.set_margin_top(18);
    let regions: Vec<&str> = view.regions.iter().map(String::as_str).collect();
    let combo = adw::ComboRow::builder()
        .title(t::licensed_region())
        .model(&gtk::StringList::new(&regions))
        .build();
    if let Some(at) = view
        .region
        .as_deref()
        .and_then(|region| regions.iter().position(|r| r.eq_ignore_ascii_case(region)))
    {
        combo.set_selected(at as u32);
    }
    let codes = view.regions.clone();
    let chosen = view.region.clone();
    combo.connect_selected_notify({
        let weak = Rc::downgrade(ctx);
        move |combo| {
            let Some(ctx) = weak.upgrade() else { return };
            let Some(code) = codes.get(combo.selected() as usize) else { return };
            if chosen.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(code)) {
                return;
            }
            ctx.hub.send(Command::Licensed(LicensedCommand::SetRegion {
                region: Some(code.clone()),
            }));
        }
    });
    region_list.append(&combo);
    column.append(&region_list);
    let region_line = widgets::haze(&t::licensed_region_line(), "caption");
    region_line.set_margin_top(6);
    column.append(&region_line);

    // Your services.
    let mut names = view.available.clone();
    for chosen in &view.settings.services {
        if !names.iter().any(|n| n.eq_ignore_ascii_case(chosen)) {
            names.push(chosen.clone());
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    let list = widgets::boxed_list();
    let all = *shown_all.borrow() || names.len() <= FIRST;
    let mut shown: Vec<&String> =
        names.iter().take(if all { names.len() } else { FIRST }).collect();
    // A ticked service is never hidden behind Show all.
    if !all {
        for name in &names {
            if view.settings.uses(name) && !shown.contains(&name) {
                shown.push(name);
            }
        }
    }
    for name in shown {
        let row = adw::ActionRow::builder().title(name).build();
        let check = gtk::CheckButton::new();
        check.set_active(view.settings.uses(name));
        check.set_valign(gtk::Align::Center);
        check.update_property(&[gtk::accessible::Property::Label(name)]);
        row.add_prefix(&check);
        row.set_activatable_widget(Some(&check));
        let (weak, now, name) =
            (Rc::downgrade(ctx), view.settings.services.clone(), name.clone());
        check.connect_toggled(move |check| {
            let Some(ctx) = weak.upgrade() else { return };
            let mut services: Vec<String> =
                now.iter().filter(|s| !s.eq_ignore_ascii_case(&name)).cloned().collect();
            if check.is_active() {
                services.push(name.clone());
            }
            ctx.hub.send(Command::Licensed(LicensedCommand::SetServices { services }));
        });
        list.append(&row);
    }
    if !all {
        let more = adw::ActionRow::builder()
            .title(t::licensed_show_all(names.len() as i64))
            .activatable(true)
            .build();
        more.add_css_class("accent-row");
        more.add_suffix(&widgets::chevron());
        let (weak, flag) = (Rc::downgrade(ctx), Rc::clone(shown_all));
        more.connect_activated(move |_| {
            *flag.borrow_mut() = true;
            if let Some(ctx) = weak.upgrade() {
                // Drawn again from what the engine last said.
                let view = ctx.hub.known.borrow().licensed.clone();
                if let Some(view) = view {
                    ctx.hub.take(vec![Event::Licensed(view)]);
                }
            }
        });
        list.append(&more);
    }
    let services = first_party::section(&t::licensed_yours(), &list);
    services.set_margin_top(20);
    column.append(&services);

    // Show other services on title pages.
    let others = widgets::boxed_list();
    others.set_margin_top(20);
    let switch = adw::SwitchRow::builder()
        .title(t::licensed_show_others())
        .subtitle(t::licensed_show_others_line())
        .active(view.settings.show_others)
        .build();
    switch.connect_active_notify({
        let weak = Rc::downgrade(ctx);
        move |switch| {
            if let Some(ctx) = weak.upgrade() {
                ctx.hub.send(Command::Licensed(LicensedCommand::SetShowOthers {
                    show: switch.is_active(),
                }));
            }
        }
    });
    others.append(&switch);
    column.append(&others);
}

/// The services' names in a line: "Hikari Stream, Tsuki TV".
pub fn names(links: &[ServiceLink]) -> String {
    links.iter().map(|l| l.service.as_str()).collect::<Vec<_>>().join(", ") // i18n-skip
}

/// The quiet lines under a title's buttons, for the case the engine worked out. Empty when
/// there is nothing to say.
pub fn lines(ctx: &Rc<Ctx>, availability: &TitleAvailability) -> Vec<gtk::Widget> {
    let mut out: Vec<gtk::Widget> = Vec::new();
    let external = |link: &ServiceLink, label: String, id: AnilistId| {
        let button = gtk::Button::new();
        let inside = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        inside.append(&gtk::Label::new(Some(&label)));
        inside.append(&gtk::Image::from_icon_name("adw-external-link-symbolic"));
        button.set_child(Some(&inside));
        button.add_css_class("flat");
        button.add_css_class("link");
        button.set_halign(gtk::Align::Start);
        button.set_action_name(Some("win.licensed-open"));
        button.set_action_target_value(Some(&target(id, &link.service, &link.url, None)));
        button.upcast::<gtk::Widget>()
    };
    match availability.case {
        AvailabilityCase::PlaysAlsoOn => {
            if let Some(first) = availability.also_on.first() {
                out.push(external(
                    first,
                    t::title_also_on_services(&names(&availability.also_on)),
                    availability.id,
                ));
            }
        }
        AvailabilityCase::OnService => {
            if let Some(first) = availability.also_on.first() {
                out.push(external(
                    first,
                    t::title_also_on_services(&names(&availability.also_on)),
                    availability.id,
                ));
            }
        }
        AvailabilityCase::NotInSources => {
            if !availability.others.is_empty() {
                let line = gtk::Box::new(gtk::Orientation::Vertical, 2);
                line.append(&widgets::haze(
                    &t::title_listed_on(&availability.others.join(", ")), // i18n-skip
                    "",
                ));
                out.push(line.upcast());
            }
            if availability.choose_services || !availability.others.is_empty() {
                let choose = gtk::Button::with_label(&t::title_choose_services());
                choose.add_css_class("flat");
                choose.add_css_class("link");
                choose.set_halign(gtk::Align::Start);
                let weak = Rc::downgrade(ctx);
                choose.connect_clicked(move |_| {
                    if let Some(ctx) = weak.upgrade() {
                        show(&ctx);
                    }
                });
                out.push(choose.upcast());
            }
        }
        AvailabilityCase::PlaysHere => {}
    }
    if availability.case != AvailabilityCase::NotInSources && !availability.others.is_empty() {
        out.push(
            widgets::haze(&t::title_others_line(&availability.others.join(", ")), "").upcast(), // i18n-skip
        );
    }
    out
}

/// The window action `licensed-open`'s target.
pub fn target(id: AnilistId, service: &str, url: &str, episode: Option<u32>) -> glib::Variant {
    (id.get(), service.to_owned(), url.to_owned(), episode.unwrap_or(0)).to_variant()
}

/// What `licensed-open` was asked to open.
pub fn open(ctx: &Ctx, target: &glib::Variant) {
    let Some((id, service, url, episode)) = <(u32, String, String, u32)>::from_variant(target)
    else {
        return;
    };
    ctx.hub.send(Command::Licensed(LicensedCommand::Open {
        id: AnilistId::new(id),
        service,
        url,
        episode: (episode > 0).then_some(episode),
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::licensed::LicensedSettings;

    fn view(services: &[&str], region: Option<&str>) -> LicensedView {
        LicensedView {
            settings: LicensedSettings {
                services: services.iter().map(|s| (*s).to_owned()).collect(),
                region: region.map(str::to_owned),
                show_others: true,
            },
            region: region.map(str::to_owned),
            available: Vec::new(),
            regions: Vec::new(),
        }
    }

    #[test]
    fn the_row_line_names_the_chosen_services_and_region() {
        let line = line(Some(&view(&["Lanternflix", "Harbourplay"], Some("DK"))));
        assert!(line.contains("Lanternflix, Harbourplay"));
        assert!(line.contains("DK"));
    }

    #[test]
    fn the_row_line_without_a_choice_asks_for_one() {
        assert_eq!(line(None), t::licensed_row_none());
        assert_eq!(line(Some(&view(&[], Some("DK")))), t::licensed_row_none());
    }

    #[test]
    fn an_episode_target_round_trips_through_the_action() {
        let target = target(AnilistId::new(7), "Lanternflix", "https://x.example/7", Some(3));
        let (id, service, url, episode) =
            <(u32, String, String, u32)>::from_variant(&target).unwrap();
        assert_eq!(
            (id, service.as_str(), url.as_str(), episode),
            (7, "Lanternflix", "https://x.example/7", 3)
        );
    }
}
