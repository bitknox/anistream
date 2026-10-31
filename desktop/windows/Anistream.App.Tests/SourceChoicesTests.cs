// The source picker's rules: its rows, how each is worded, what a choice does to the listing,
// the one notice a pin owes, and which episode the main button plays when some have no source.

using Anistream.App;
using Anistream.Engine.Core;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Xunit;

namespace Anistream.App.Tests;

[Collection("Strings")]
public class SourceChoicesTests
{
    public SourceChoicesTests()
    {
        T.Resolver = null;
        T.Language = "en";
    }

    static SourceCandidate Release(string id, string title, uint? quality = null, string? label = null, string? size = null) =>
        new(id, "torrent", title, quality, size, label);

    static SourceOption Option(string id, uint covered, SourceNote? note = null, bool pinned = false,
        SourceCandidate[]? releases = null, string? pinnedRelease = null) =>
        new(id, $"Source {id}", covered, 13, pinned, note, releases ?? [], pinnedRelease);

    [Fact]
    public void Automatic_stands_alone_until_the_engine_lists_sources()
    {
        var rows = SourcePicker.Rows(null);
        Assert.Equal([new PickerRow.Automatic(false)], rows);
    }

    [Fact]
    public void Automatic_is_chosen_when_no_source_is_pinned()
    {
        var rows = SourcePicker.Rows([Option("a", 13), Option("b", 6)]);
        Assert.Equal(new PickerRow.Automatic(true), rows[0]);
        Assert.Equal(["automatic", "source/a", "source/b"], rows.Select(r => r.Key));
    }

    [Fact]
    public void The_pinned_source_is_the_chosen_row()
    {
        var rows = SourcePicker.Rows([Option("a", 13), Option("b", 6, pinned: true)]);
        Assert.Equal(new PickerRow.Automatic(false), rows[0]);
        Assert.False(((PickerRow.Source)rows[1]).Chosen);
        Assert.True(((PickerRow.Source)rows[2]).Chosen);
    }

    [Fact]
    public void A_source_with_several_releases_lists_them_under_a_heading()
    {
        var releases = new[] { Release("batch", "Batch"), Release("one", "Single") };
        var rows = SourcePicker.Rows([Option("t", 6, releases: releases)]);
        Assert.Equal(["automatic", "source/t", "releases/t", "release/t/batch", "release/t/one"], rows.Select(r => r.Key));
    }

    [Fact]
    public void A_source_with_one_release_has_no_heading()
    {
        var rows = SourcePicker.Rows([Option("t", 6, releases: [Release("only", "Only")])]);
        Assert.Equal(["automatic", "source/t"], rows.Select(r => r.Key));
    }

    [Fact]
    public void A_pinned_release_is_checked_in_place_of_its_source()
    {
        var releases = new[] { Release("batch", "Batch"), Release("one", "Single") };
        var rows = SourcePicker.Rows([Option("t", 6, pinned: true, releases: releases, pinnedRelease: "one")]);
        Assert.False(((PickerRow.Source)rows[1]).Chosen);
        Assert.False(((PickerRow.Release)rows[3]).Chosen);
        Assert.True(((PickerRow.Release)rows[4]).Chosen);
    }

    [Fact]
    public void A_pinned_release_the_listing_lost_leaves_the_source_checked()
    {
        var releases = new[] { Release("batch", "Batch"), Release("one", "Single") };
        var rows = SourcePicker.Rows([Option("t", 6, pinned: true, releases: releases, pinnedRelease: "gone")]);
        Assert.True(((PickerRow.Source)rows[1]).Chosen);
    }

    [Fact]
    public void A_source_is_worded_by_its_coverage_or_its_note()
    {
        Assert.Equal("6 of 13 episodes", SourcePicker.Detail(Option("a", 6)));
        Assert.Equal("1 of 1 episode", SourcePicker.Detail(new("a", "A", 1, 1)));
        Assert.Equal("Switched off", SourcePicker.Detail(Option("a", 0, new SourceNote.Off())));
        Assert.Equal("Not responding, trying again soon", SourcePicker.Detail(Option("a", 0, new SourceNote.Resting())));
        Assert.Equal("Not installed", SourcePicker.Detail(Option("a", 0, new SourceNote.Gone())));
        Assert.Equal("Couldn't match this title", SourcePicker.Detail(Option("a", 0, new SourceNote.Ambiguous())));
    }

    [Fact]
    public void A_failure_is_the_sources_own_words()
    {
        Assert.Equal("the site did not answer in 15 s.", SourcePicker.Detail(Option("a", 0, new SourceNote.Failed("the site did not answer in 15 s."))));
    }

    [Fact]
    public void A_source_that_is_no_longer_installed_cannot_be_picked()
    {
        var rows = SourcePicker.Rows([Option("a", 0, new SourceNote.Gone(), pinned: true), Option("b", 3, new SourceNote.Off())]);
        Assert.False(((PickerRow.Source)rows[1]).Selectable);
        Assert.True(((PickerRow.Source)rows[2]).Selectable);
        Assert.True(((PickerRow.Source)rows[2]).Dimmed);
    }

    [Fact]
    public void A_row_is_spoken_with_its_coverage_and_whether_it_is_chosen()
    {
        var rows = SourcePicker.Rows([Option("a", 13, pinned: true), Option("b", 6)]);
        Assert.Equal("Automatic, Each episode from the first source that has it", SourcePicker.Spoken(rows[0]));
        Assert.Equal("Source a, 13 of 13 episodes, Chosen", SourcePicker.Spoken(rows[1]));
        Assert.Equal("Source b, 6 of 13 episodes", SourcePicker.Spoken(rows[2]));
    }

    [Fact]
    public void A_release_says_its_quality_size_and_label()
    {
        Assert.Equal("1080p · 14.2 GiB · 84 seeders", SourcePicker.ReleaseDetail(Release("r", "R", 1080, "84 seeders", "14.2 GiB")));
        Assert.Equal("One seeder", SourcePicker.ReleaseDetail(Release("r", "R", label: "One seeder")));
        Assert.Null(SourcePicker.ReleaseDetail(Release("r", "R")));
    }

    [Fact]
    public void A_listing_waits_for_the_engine_and_a_choice_follows_at_once()
    {
        var choices = new SourceChoices();
        Assert.Null(choices.For(7));
        choices.Asking(7);
        Assert.True(choices.Loading(7));
        choices.Take(new Event.SourceOptions(7, [Option("a", 13), Option("b", 6)]));
        Assert.False(choices.Loading(7));

        choices.Chose(7, "b");
        Assert.Equal([false, true], choices.For(7)!.Select(o => o.Pinned));
        choices.Chose(7, null);
        Assert.Equal([false, false], choices.For(7)!.Select(o => o.Pinned));
        choices.Chose(7, "a", "batch");
        Assert.Equal("batch", choices.For(7)![0].PinnedRelease);
        Assert.Null(choices.For(7)![1].PinnedRelease);
    }

    [Fact]
    public void A_picker_closed_before_the_answer_stops_waiting()
    {
        var choices = new SourceChoices();
        choices.Asking(7);
        choices.GiveUp(7);
        Assert.False(choices.Loading(7));
    }

    [Fact]
    public void A_pin_that_cannot_be_used_is_said_once_until_the_choice_changes()
    {
        var choices = new SourceChoices();
        var notice = new PinUnavailableNotice(7, "a", "Source a");
        Assert.True(choices.Say(notice));
        Assert.False(choices.Say(notice));
        Assert.True(choices.Say(notice with { Id = 8 }));
        choices.Chose(7, "a");
        Assert.True(choices.Say(notice));
    }

    static EpisodeRow[] Table(params (string Number, bool Unsourced)[] rows) =>
        [.. rows.Select(r => new EpisodeRow(r.Number, Unsourced: r.Unsourced))];

    static Entry Watching(uint next, ResumePoint? resume = null) =>
        new(Id: 1, Title: "Sample", Episodes: 13, Progress: new TitleProgress(next - 1, next), Resume: resume);

    [Fact]
    public void The_button_plays_the_next_episode_when_a_source_lists_it()
    {
        var rows = Table(("3", false), ("4", false), ("5", true));
        Assert.Equal(new PlayableNext(4, false), PlayableNext.Of(Watching(4), rows));
        Assert.Equal(new PlayableNext(4, false), PlayableNext.Of(Watching(4), null));
    }

    [Fact]
    public void The_button_skips_to_the_next_episode_a_source_lists()
    {
        var rows = Table(("4", true), ("5", true), ("6", false), ("7", false));
        Assert.Equal(new PlayableNext(6, false), PlayableNext.Of(Watching(4), rows));
    }

    [Fact]
    public void The_button_has_nothing_to_play_when_every_later_episode_is_unsourced()
    {
        var rows = Table(("3", false), ("4", true), ("5", true));
        Assert.Equal(new PlayableNext(4, true), PlayableNext.Of(Watching(4), rows));
    }

    [Fact]
    public void A_part_watched_episode_is_resumed_as_it_is()
    {
        var rows = Table(("4", true), ("5", false));
        Assert.Equal(new PlayableNext(4, false), PlayableNext.Of(Watching(4, new ResumePoint(300, 0.2)), rows));
    }

    [Fact]
    public void A_title_with_no_progress_starts_at_the_first_episode_a_source_lists()
    {
        var entry = new Entry(Id: 1, Title: "Sample", Episodes: 13);
        Assert.Equal(new PlayableNext(2, false), PlayableNext.Of(entry, Table(("1", true), ("2", false))));
    }

    [Fact]
    public void Unsourced_episodes_are_neither_played_nor_downloaded()
    {
        var rows = Table(("1", false), ("2", true), ("3", false));
        Assert.True(EpisodeSources.Playable(rows[0]));
        Assert.False(EpisodeSources.Playable(rows[1]));
        Assert.False(EpisodeSources.Playable(new EpisodeRow("4", MetadataOnly: true)));
        Assert.Equal(["1", "3"], EpisodeSources.Downloadable(rows));
    }

    [Fact]
    public void The_main_button_names_the_episode_it_skips_to()
    {
        var entry = Watching(4) with { Status = AiringStatus.Finished, LastAired = null };
        var rows = Table(("4", true), ("5", true), ("6", false));
        Assert.Equal("Play episode 4", PlayableNext.Item(entry, null, Casing.Sentence).Button);
        Assert.Equal("Play episode 6", PlayableNext.Item(entry, rows, Casing.Sentence).Button);
    }

    [Fact]
    public void With_nothing_to_play_the_row_says_no_source_has_it()
    {
        var entry = Watching(4) with { Status = AiringStatus.Finished };
        var item = PlayableNext.Item(entry, Table(("4", true), ("5", true)), Casing.Sentence);
        Assert.False(item.Playable);
        Assert.Null(item.Button);
        Assert.Equal("No source has this episode", item.Waiting);
    }
}
