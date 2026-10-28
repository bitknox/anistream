use super::*;

#[test]
fn a_comment_s_age_is_worded_as_a_timeline_words_it() {
    assert_eq!(age(-5), Msg::DiscussionAgeNow, "a clock a little behind is now");
    assert_eq!(age(59), Msg::DiscussionAgeNow);
    assert_eq!(age(5 * 60), Msg::DiscussionAgeMinutes { minutes: 5 });
    assert_eq!(age(2 * 3_600 + 59), Msg::DiscussionAgeHours { hours: 2 });
    assert_eq!(age(3 * 86_400), Msg::DiscussionAgeDays { days: 3 });
    assert_eq!(age(29 * 86_400), Msg::DiscussionAgeWeeks { weeks: 4 });
    assert_eq!(age(800 * 86_400), Msg::DiscussionAgeYears { years: 2 });
}
