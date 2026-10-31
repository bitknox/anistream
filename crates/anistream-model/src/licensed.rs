//! Licensed services: titles the user can open in a streaming service they pay for.
//!
//! anistream never plays these. AniList lists which services carry a title, and this module works
//! out what a title page should say about them, so every app draws the same thing. It has no I/O:
//! the engine hands it the links, the viewer's choices and whether the title plays in anistream.

use anistream_core::ids::AnilistId;

/// What the viewer chose on the Licensed services page.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct LicensedSettings {
    /// The services they pay for, as AniList spells them.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub services: Vec<String>,
    /// The country they chose, as an ISO 3166 code such as `DK`. `None` follows the system.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub region: Option<String>,
    /// Whether a title page names services the viewer does not use.
    #[cfg_attr(feature = "ffi", uniffi(default = true))]
    pub show_others: bool,
}

impl Default for LicensedSettings {
    fn default() -> Self {
        Self { services: Vec::new(), region: None, show_others: true }
    }
}

impl LicensedSettings {
    /// Whether `service` is one the viewer pays for. Names compare without regard to case.
    pub fn uses(&self, service: &str) -> bool {
        self.services.iter().any(|s| s.eq_ignore_ascii_case(service))
    }
}

/// The Licensed services page: the choices, and the services AniList knows to choose from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct LicensedView {
    pub settings: LicensedSettings,
    /// The region links are hidden by: the chosen one, or else the system's. `None` when neither
    /// is known, and then no link is hidden.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub region: Option<String>,
    /// Every streaming service AniList lists, sorted by name. The page shows the first eight and
    /// "Show all" for the rest. Empty until AniList has answered once.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub available: Vec<String>,
    /// The country codes Region offers, sorted.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub regions: Vec<String>,
}

/// A service's page for a title.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ServiceLink {
    /// The service's name, as AniList spells it.
    pub service: String,
    /// An `http` or `https` address.
    pub url: String,
    /// AniList's free text about the link, which sometimes names a country.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub notes: Option<String>,
}

/// One episode's page on a service.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EpisodeLink {
    pub episode: u32,
    pub service: String,
    pub url: String,
}

/// Which of the title page's three cases a title is in, plus the one where nothing needs saying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum AvailabilityCase {
    /// It plays in anistream and is on none of the viewer's services. Nothing extra is shown.
    PlaysHere,
    /// Case A. It plays in anistream and is also on a service the viewer uses.
    PlaysAlsoOn,
    /// Case B. It does not play in anistream and is on a service the viewer uses: the main action
    /// is Watch on that service.
    OnService,
    /// Case C. It is on none of the viewer's services and does not play in anistream.
    NotInSources,
}

/// What a title page says about licensed services.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TitleAvailability {
    pub id: AnilistId,
    pub case: AvailabilityCase,
    /// Case B: the service the main action opens.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub main: Option<ServiceLink>,
    /// Case A: the viewer's services that have the title, for "Also on". Case B: the ones after
    /// the main service.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub also_on: Vec<ServiceLink>,
    /// Every one of the viewer's services that has the title, for the More menu's Watch on items.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub yours: Vec<ServiceLink>,
    /// Services the viewer does not use that have the title. Case C always names them. Cases A
    /// and B name them only while "Show other services" is on.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub others: Vec<String>,
    /// Case B: each episode's own page on the main service, where the service published one. An
    /// episode without one opens the title's link.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episodes: Vec<EpisodeLink>,
    /// The viewer has chosen no service yet, so the page offers Choose your services.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub choose_services: bool,
}

/// A viewer left a title for a service's page and has come back after long enough to have watched
/// an episode. The app asks "Watched episode N on X?" and offers Mark as watched, which is
/// `Command::SetWatched` for the episode.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct LicensedReturn {
    pub id: AnilistId,
    pub episode: u32,
    pub service: String,
    /// "Watched episode 4 on Hikari Stream?"
    pub prompt: crate::Msg,
    /// "Episode 4 marked as watched.", for the notice once it is marked.
    pub done: crate::Msg,
}

/// How long the viewer has to be away for the return notice to ask. Shorter than an episode, so a
/// glance at a service's page asks nothing.
pub const AWAY_FOR_SECS: i64 = 10 * 60;

/// Country codes, ISO 3166-1 alpha-2. Region offers these, and a link's notes naming one of them
/// is read as the country the link works in.
pub const REGIONS: &[&str] = &[
    "AD", "AE", "AF", "AG", "AI", "AL", "AM", "AO", "AQ", "AR", "AS", "AT", "AU", "AW", "AX",
    "AZ", "BA", "BB", "BD", "BE", "BF", "BG", "BH", "BI", "BJ", "BL", "BM", "BN", "BO", "BQ",
    "BR", "BS", "BT", "BV", "BW", "BY", "BZ", "CA", "CC", "CD", "CF", "CG", "CH", "CI", "CK",
    "CL", "CM", "CN", "CO", "CR", "CU", "CV", "CW", "CX", "CY", "CZ", "DE", "DJ", "DK", "DM",
    "DO", "DZ", "EC", "EE", "EG", "EH", "ER", "ES", "ET", "FI", "FJ", "FK", "FM", "FO", "FR",
    "GA", "GB", "GD", "GE", "GF", "GG", "GH", "GI", "GL", "GM", "GN", "GP", "GQ", "GR", "GS",
    "GT", "GU", "GW", "GY", "HK", "HM", "HN", "HR", "HT", "HU", "ID", "IE", "IL", "IM", "IN",
    "IO", "IQ", "IR", "IS", "IT", "JE", "JM", "JO", "JP", "KE", "KG", "KH", "KI", "KM", "KN",
    "KP", "KR", "KW", "KY", "KZ", "LA", "LB", "LC", "LI", "LK", "LR", "LS", "LT", "LU", "LV",
    "LY", "MA", "MC", "MD", "ME", "MF", "MG", "MH", "MK", "ML", "MM", "MN", "MO", "MP", "MQ",
    "MR", "MS", "MT", "MU", "MV", "MW", "MX", "MY", "MZ", "NA", "NC", "NE", "NF", "NG", "NI",
    "NL", "NO", "NP", "NR", "NU", "NZ", "OM", "PA", "PE", "PF", "PG", "PH", "PK", "PL", "PM",
    "PN", "PR", "PS", "PT", "PW", "PY", "QA", "RE", "RO", "RS", "RU", "RW", "SA", "SB", "SC",
    "SD", "SE", "SG", "SH", "SI", "SJ", "SK", "SL", "SM", "SN", "SO", "SR", "SS", "ST", "SV",
    "SX", "SY", "SZ", "TC", "TD", "TF", "TG", "TH", "TJ", "TK", "TL", "TM", "TN", "TO", "TR",
    "TT", "TV", "TW", "TZ", "UA", "UG", "UM", "US", "UY", "UZ", "VA", "VC", "VE", "VG", "VI",
    "VN", "VU", "WF", "WS", "YE", "YT", "ZA", "ZM", "ZW",
];

/// The countries a link's notes name: each whole word that is a country code, in capitals.
fn countries_named(notes: &str) -> Vec<&str> {
    notes
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|word| word.len() == 2 && word.bytes().all(|b| b.is_ascii_uppercase()))
        .filter(|word| REGIONS.contains(word))
        .collect()
}

/// Whether a link's notes rule it out for `region`: they name countries, and not that one. A link
/// whose notes name no country is kept, because most say nothing about where they work.
pub fn hidden_by_region(notes: Option<&str>, region: Option<&str>) -> bool {
    let (Some(notes), Some(region)) = (notes, region) else { return false };
    let named = countries_named(notes);
    !named.is_empty() && !named.iter().any(|c| c.eq_ignore_ascii_case(region))
}

/// Work out what a title page says. `links` are the title's streaming links with the disabled and
/// non-web ones already left out; `listings` are the episodes' own pages. `region` is the country
/// links are hidden by.
pub fn resolve_availability(
    id: AnilistId,
    links: &[ServiceLink],
    listings: &[EpisodeLink],
    settings: &LicensedSettings,
    region: Option<&str>,
    plays_here: bool,
) -> TitleAvailability {
    let mut yours: Vec<ServiceLink> = Vec::new();
    let mut others: Vec<String> = Vec::new();
    for link in links.iter().filter(|l| !hidden_by_region(l.notes.as_deref(), region)) {
        if settings.uses(&link.service) {
            if !yours.iter().any(|l| l.service.eq_ignore_ascii_case(&link.service)) {
                yours.push(link.clone());
            }
        } else if !others.iter().any(|s| s.eq_ignore_ascii_case(&link.service)) {
            others.push(link.service.clone());
        }
    }
    let choose_services = settings.services.is_empty();
    let (case, main, also_on, episodes) = match (yours.is_empty(), plays_here) {
        (false, true) => (AvailabilityCase::PlaysAlsoOn, None, yours.clone(), Vec::new()),
        (false, false) => {
            let main = yours[0].clone();
            let episodes = listings
                .iter()
                .filter(|l| l.service.eq_ignore_ascii_case(&main.service))
                .cloned()
                .collect();
            (AvailabilityCase::OnService, Some(main), yours[1..].to_vec(), episodes)
        }
        (true, true) => (AvailabilityCase::PlaysHere, None, Vec::new(), Vec::new()),
        (true, false) => (AvailabilityCase::NotInSources, None, Vec::new(), Vec::new()),
    };
    let show_others = settings.show_others || case == AvailabilityCase::NotInSources;
    TitleAvailability {
        id,
        case,
        main,
        also_on,
        yours,
        others: if show_others { others } else { Vec::new() },
        episodes,
        choose_services,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: AnilistId = AnilistId::new(900_001);

    fn link(service: &str, notes: Option<&str>) -> ServiceLink {
        ServiceLink {
            service: service.into(),
            url: format!("https://{}.example/title", service.to_lowercase().replace(' ', "-")),
            notes: notes.map(String::from),
        }
    }

    fn uses(services: &[&str]) -> LicensedSettings {
        LicensedSettings {
            services: services.iter().map(|s| (*s).into()).collect(),
            ..LicensedSettings::default()
        }
    }

    #[test]
    fn a_title_that_plays_here_and_is_on_your_service_says_also_on() {
        let links = [link("Hikari Stream", None), link("Tsuki TV", None)];
        let a = resolve_availability(ID, &links, &[], &uses(&["Hikari Stream"]), None, true);
        assert_eq!(a.case, AvailabilityCase::PlaysAlsoOn);
        assert_eq!(a.also_on.len(), 1);
        assert!(a.main.is_none());
        assert_eq!(a.others, ["Tsuki TV"]);
    }

    #[test]
    fn a_title_that_does_not_play_here_opens_your_first_service() {
        let links =
            [link("Tsuki TV", None), link("Hikari Stream", None), link("Aozora Plus", None)];
        let listings = [
            EpisodeLink {
                episode: 1,
                service: "Hikari Stream".into(),
                url: "https://a.example/1".into(),
            },
            EpisodeLink {
                episode: 1,
                service: "Aozora Plus".into(),
                url: "https://b.example/1".into(),
            },
        ];
        let b = resolve_availability(
            ID,
            &links,
            &listings,
            &uses(&["hikari stream", "Tsuki TV"]),
            None,
            false,
        );
        assert_eq!(b.case, AvailabilityCase::OnService);
        assert_eq!(b.main.as_ref().unwrap().service, "Tsuki TV");
        assert_eq!(b.also_on.len(), 1);
        assert_eq!(b.yours.len(), 2);
        assert!(
            b.episodes.is_empty(),
            "only the main service's episode pages: {:?}",
            b.episodes
        );
        assert_eq!(b.others, ["Aozora Plus"]);
    }

    #[test]
    fn episode_pages_follow_the_main_service() {
        let links = [link("Hikari Stream", None)];
        let listings = [EpisodeLink {
            episode: 3,
            service: "Hikari Stream".into(),
            url: "https://a.example/3".into(),
        }];
        let b =
            resolve_availability(ID, &links, &listings, &uses(&["Hikari Stream"]), None, false);
        assert_eq!(b.episodes.len(), 1);
    }

    #[test]
    fn a_title_on_none_of_your_services_lists_where_it_is() {
        let links = [link("Aozora Plus", None), link("Minato Video", None)];
        let mut settings = uses(&["Hikari Stream"]);
        settings.show_others = false;
        let c = resolve_availability(ID, &links, &[], &settings, None, false);
        assert_eq!(c.case, AvailabilityCase::NotInSources);
        // Case C says where it is listed even with the switch off: it is the whole message.
        assert_eq!(c.others, ["Aozora Plus", "Minato Video"]);
        assert!(!c.choose_services);
    }

    #[test]
    fn the_switch_hides_other_services_in_the_other_cases() {
        let links = [link("Hikari Stream", None), link("Tsuki TV", None)];
        let mut settings = uses(&["Hikari Stream"]);
        settings.show_others = false;
        assert!(resolve_availability(ID, &links, &[], &settings, None, true).others.is_empty());
        assert!(
            resolve_availability(ID, &links, &[], &settings, None, false).others.is_empty()
        );
    }

    #[test]
    fn until_a_service_is_chosen_there_are_no_watch_on_buttons() {
        let links = [link("Hikari Stream", None)];
        let none =
            resolve_availability(ID, &links, &[], &LicensedSettings::default(), None, false);
        assert_eq!(none.case, AvailabilityCase::NotInSources);
        assert!(none.choose_services);
        assert!(none.main.is_none());
    }

    #[test]
    fn region_hides_links_whose_notes_name_only_other_countries() {
        assert!(hidden_by_region(Some("JP"), Some("DK")));
        assert!(!hidden_by_region(Some("JP"), Some("JP")));
        assert!(!hidden_by_region(Some("JP, DK"), Some("dk")));
        // Words that are not countries, and no notes at all, say nothing about where it works.
        assert!(!hidden_by_region(Some("Channel"), Some("DK")));
        assert!(!hidden_by_region(Some("EN"), Some("DK")));
        assert!(!hidden_by_region(None, Some("DK")));
        assert!(!hidden_by_region(Some("JP"), None));
    }

    #[test]
    fn a_hidden_link_is_not_listed_anywhere() {
        let links = [link("Hikari Stream", Some("JP")), link("Tsuki TV", None)];
        let a =
            resolve_availability(ID, &links, &[], &uses(&["Hikari Stream"]), Some("DK"), false);
        assert_eq!(a.case, AvailabilityCase::NotInSources);
        assert_eq!(a.others, ["Tsuki TV"]);
    }

    #[test]
    fn the_region_list_is_sorted_and_unique() {
        let mut sorted = REGIONS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, REGIONS);
    }
}
