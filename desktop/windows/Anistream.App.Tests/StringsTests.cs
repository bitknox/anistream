// The generated strings (tools/i18n writes Anistream.App.Core/Strings.g.cs): the English they
// carry, the plural forms, and the answer for a key the resource loader lacks.

using Anistream.App;
using Xunit;

namespace Anistream.App.Tests;

public class StringsTests
{
    [Fact]
    public void English_is_compiled_in_for_a_start_without_resources()
    {
        T.Resolver = null;
        T.Language = "en";
        Assert.Equal("Language", T.SettingsLanguage);
        Assert.Equal("Try again", T.ActionTryAgain);
        Assert.Equal("Sources", T.SourcesTitle);
    }

    [Fact]
    public void A_plural_follows_the_count_and_fills_its_arguments()
    {
        T.Resolver = null;
        T.Language = "en";
        Assert.Equal("One episode left in Sample", T.QueueEpisodesLeft(1, "Sample"));
        Assert.Equal("5 episodes left in Sample", T.QueueEpisodesLeft(5, "Sample"));
        Assert.Equal("6 of 13 episodes", T.ProgressEpisodes(6, 13));
        Assert.Equal("6 of 1 episode", T.ProgressEpisodes(6, 1));
    }

    [Fact]
    public void The_resolver_wins_and_a_missing_key_falls_back_to_english()
    {
        T.Language = "en";
        T.Resolver = key => key == "SettingsLanguage" ? "Sprog" : null;
        try
        {
            Assert.Equal("Sprog", T.SettingsLanguage);
            Assert.Equal("Try again", T.ActionTryAgain);
        }
        finally
        {
            T.Resolver = null;
        }
    }

    [Fact]
    public void Plural_categories_come_from_cldr()
    {
        Assert.Equal("one", T.Category("en", 1));
        Assert.Equal("other", T.Category("en", 0));
        Assert.Equal("other", T.Category("en", 1_000_000));
        Assert.Equal("one", T.Category("en-GB", 1));
    }

    [Fact]
    public void Braces_in_text_are_literal_and_unknown_placeholders_stay()
    {
        T.Resolver = key => key == "NoticeSaved" ? "{{x}} {label} {nope}" : null;
        T.Language = "en";
        try
        {
            Assert.Equal("{x} Language {nope}", T.NoticeSaved("Language"));
        }
        finally
        {
            T.Resolver = null;
        }
    }
}
