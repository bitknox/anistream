//! Opening and ending skip times, from aniskip.
//!
//! Verified during planning: `api.aniskip.com/v2` returns real intervals — Frieren episode 1
//! gives OP `3.2–93.2s` and ED `1417–1507s`. It is keyed on **MAL id**, not AniList, which is
//! one of the two places the mapping layer earns its keep (the other being release numbering).
//!
//! A title with no MAL id simply has no skip data. The prompt is then absent rather than
//! broken — this is decoration, and an episode plays perfectly well without it.

use serde::Deserialize;

/// Which segment a skip interval covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipKind {
    Opening,
    Ending,
}

impl SkipKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Opening => "opening",
            Self::Ending => "ending",
        }
    }

    const fn api_type(self) -> &'static str {
        match self {
            Self::Opening => "op",
            Self::Ending => "ed",
        }
    }
}

/// A skippable segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkipInterval {
    pub kind: SkipKind,
    pub start: f64,
    pub end: f64,
}

impl SkipInterval {
    /// Whether the playhead is inside this segment.
    ///
    /// The start is nudged forward slightly: offering to skip an opening in its first instant
    /// is jarring, and a viewer who just seeked there probably meant to be there.
    pub fn contains(&self, position: f64) -> bool {
        position >= self.start + 0.5 && position < self.end
    }

    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

/// Query URL for one episode's skip times.
pub fn query_url(mal_id: u32, episode: u32) -> String {
    format!(
        "https://api.aniskip.com/v2/skip-times/{mal_id}/{episode}\
         ?types={}&types={}&episodeLength=0",
        SkipKind::Opening.api_type(),
        SkipKind::Ending.api_type()
    )
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    found: bool,
    #[serde(default)]
    results: Vec<Result_>,
}

#[derive(Deserialize)]
struct Result_ {
    interval: Interval,
    #[serde(rename = "skipType")]
    skip_type: String,
}

#[derive(Deserialize)]
struct Interval {
    #[serde(rename = "startTime")]
    start_time: f64,
    #[serde(rename = "endTime")]
    end_time: f64,
}

/// Parse an aniskip response.
///
/// Anything unexpected yields no intervals rather than an error: skip data is optional, and a
/// changed API shape should cost the prompt, not the episode.
pub fn parse(payload: &str) -> Vec<SkipInterval> {
    let Ok(response) = serde_json::from_str::<Response>(payload) else {
        return Vec::new();
    };
    if !response.found {
        return Vec::new();
    }

    response
        .results
        .into_iter()
        .filter_map(|result| {
            let kind = match result.skip_type.as_str() {
                "op" | "mixed-op" => SkipKind::Opening,
                "ed" | "mixed-ed" => SkipKind::Ending,
                _ => return None,
            };
            let interval = SkipInterval {
                kind,
                start: result.interval.start_time,
                end: result.interval.end_time,
            };
            // A zero-length or backwards interval would produce a prompt that skips nowhere.
            (interval.duration() > 1.0).then_some(interval)
        })
        .collect()
}

/// The segment the playhead is currently inside, if any.
pub fn active(intervals: &[SkipInterval], position: f64) -> Option<&SkipInterval> {
    intervals.iter().find(|i| i.contains(position))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape captured from api.aniskip.com during planning (Frieren, MAL 52991, ep 1).
    const PAYLOAD: &str = r#"{
      "found": true,
      "results": [
        {"interval":{"startTime":3.221,"endTime":93.221},
         "skipType":"op","skipId":"c2cacbe5","episodeLength":1559.949},
        {"interval":{"startTime":1417.135,"endTime":1507.135},
         "skipType":"ed","skipId":"92965d0e","episodeLength":1510.102}
      ],
      "message":"Successfully found skip times","statusCode":200
    }"#;

    #[test]
    fn the_query_is_keyed_on_mal_id_not_anilist() {
        // Getting this wrong returns skip times for an unrelated show.
        let url = query_url(52_991, 1);
        assert!(url.contains("/skip-times/52991/1"));
        assert!(url.contains("types=op"));
        assert!(url.contains("types=ed"));
    }

    #[test]
    fn a_real_payload_yields_both_intervals() {
        let intervals = parse(PAYLOAD);
        assert_eq!(intervals.len(), 2);

        let opening = intervals.iter().find(|i| i.kind == SkipKind::Opening).unwrap();
        assert!((opening.start - 3.221).abs() < 1e-6);
        assert!((opening.end - 93.221).abs() < 1e-6);
        assert!((opening.duration() - 90.0).abs() < 0.01);

        let ending = intervals.iter().find(|i| i.kind == SkipKind::Ending).unwrap();
        assert!((ending.start - 1417.135).abs() < 1e-6);
    }

    #[test]
    fn a_not_found_response_yields_nothing() {
        // The common case — most titles have no submitted skip times.
        let payload =
            r#"{"found":false,"results":[],"message":"No skip times found","statusCode":404}"#;
        assert!(parse(payload).is_empty());
    }

    #[test]
    fn a_changed_or_broken_api_costs_the_prompt_not_the_episode() {
        for payload in ["", "not json", "{}", r#"{"found":true}"#, "null", "[]"] {
            assert!(parse(payload).is_empty(), "unexpected result from {payload:?}");
        }
    }

    #[test]
    fn degenerate_intervals_are_discarded() {
        // A zero-length interval would produce a prompt that skips nowhere.
        let payload = r#"{"found":true,"results":[
            {"interval":{"startTime":10.0,"endTime":10.0},"skipType":"op"},
            {"interval":{"startTime":50.0,"endTime":20.0},"skipType":"ed"}
        ]}"#;
        assert!(parse(payload).is_empty());
    }

    #[test]
    fn unknown_skip_types_are_ignored() {
        let payload = r#"{"found":true,"results":[
            {"interval":{"startTime":0.0,"endTime":90.0},"skipType":"recap"}
        ]}"#;
        assert!(parse(payload).is_empty());
    }

    #[test]
    fn mixed_op_and_ed_types_are_accepted() {
        // aniskip uses these when the opening is blended into the episode.
        let payload = r#"{"found":true,"results":[
            {"interval":{"startTime":0.0,"endTime":90.0},"skipType":"mixed-op"}
        ]}"#;
        assert_eq!(parse(payload)[0].kind, SkipKind::Opening);
    }

    #[test]
    fn the_active_interval_tracks_the_playhead() {
        let intervals = parse(PAYLOAD);

        // Before the opening.
        assert!(active(&intervals, 1.0).is_none());
        // Inside it.
        assert_eq!(active(&intervals, 30.0).map(|i| i.kind), Some(SkipKind::Opening));
        // After it, before the ending.
        assert!(active(&intervals, 200.0).is_none());
        // Inside the ending.
        assert_eq!(active(&intervals, 1450.0).map(|i| i.kind), Some(SkipKind::Ending));
        // Past everything.
        assert!(active(&intervals, 1509.0).is_none());
    }

    #[test]
    fn the_prompt_does_not_appear_in_the_first_instant() {
        // Offering to skip an opening the moment it starts is jarring, and someone who just
        // seeked there probably meant to be there.
        let intervals = parse(PAYLOAD);
        assert!(active(&intervals, 3.3).is_none(), "too eager");
        assert!(active(&intervals, 4.0).is_some());
    }

    #[test]
    fn the_end_of_an_interval_is_exclusive() {
        // Otherwise the prompt lingers for a frame after the segment has passed.
        let intervals = parse(PAYLOAD);
        assert!(active(&intervals, 93.221).is_none());
        assert!(active(&intervals, 93.0).is_some());
    }
}
