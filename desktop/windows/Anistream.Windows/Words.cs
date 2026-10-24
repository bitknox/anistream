using System.Text;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;

namespace Anistream.Windows;

/// <summary>
/// The few words the model has no rule for yet, in sentence case as Windows writes them.
/// Everything else comes from the model's own rules (AnistreamFfiMethods).
/// </summary>
static class Words
{
    /// <summary>"LightNovel" or "LIGHT_NOVEL" as "Light novel".</summary>
    public static string Sentence(string name)
    {
        var words = new StringBuilder();
        for (var i = 0; i < name.Length; i++)
        {
            var c = name[i];
            if (c == '_')
            {
                words.Append(' ');
                continue;
            }
            if (i > 0 && char.IsUpper(c) && char.IsLower(name[i - 1]))
                words.Append(' ');
            words.Append(words.Length == 0 ? char.ToUpperInvariant(c) : char.ToLowerInvariant(c));
        }
        return words.ToString();
    }

    public static string Source(SourceMaterial source) => Sentence(source.ToString());

    /// <summary>"Sequel · TV".</summary>
    public static string Relation(string relation, string? format) =>
        format is { Length: > 0 } f ? $"{Sentence(relation)} · {AnistreamFfiMethods.FormatLabel(f)}" : Sentence(relation);

    /// <summary>A setting's label as a row's title: "Auto-play next".</summary>
    public static string SettingTitle(SettingKey key)
    {
        var label = AnistreamFfiMethods.SettingLabel(key);
        return label.Length == 0 ? label : char.ToUpperInvariant(label[0]) + label[1..];
    }

    /// <summary>
    /// The engine's lower-case error chain as a screen shows it: its first clause, capitalised,
    /// as a sentence. The whole chain goes to the log.
    /// </summary>
    public static string Failure(string reason)
    {
        var first = reason.Split(": ", 2)[0].Trim().TrimEnd('.');
        if (first.Length == 0)
            return "Something went wrong.";
        return char.ToUpperInvariant(first[0]) + first[1..] + ".";
    }

    /// <summary>The engine's text with its first letter capitalised and nothing else changed.</summary>
    public static string Capital(string text) =>
        text.Length == 0 ? text : char.ToUpperInvariant(text[0]) + text[1..];

    /// <summary>
    /// A notice as the toast says it: the first letter capitalised, and the terminal's dash
    /// between two clauses as a full stop, since the apps write no dashes.
    /// </summary>
    public static string Notice(string text)
    {
        var parts = text.Split(" — ");
        return string.Join(". ", parts.Select(p => Capital(p.Trim())));
    }

    /// <summary>Why an invite would not join, in a line that says what to do next.</summary>
    public static string InviteFailure(string reason)
    {
        if (reason.StartsWith("that invite was already used", StringComparison.Ordinal))
            return "This invite was already used, cancelled or has expired. Ask the store's owner for a new one.";
        if (reason.StartsWith("that link no longer lets anyone join", StringComparison.Ordinal))
            return "This link no longer lets anyone join. Ask the store's owner for a new link or an invite.";
        if (reason.Contains("not a link", StringComparison.Ordinal) || reason.Contains("invite", StringComparison.Ordinal) && reason.Contains("parse", StringComparison.Ordinal))
            return "That is not an invite link.";
        return "Couldn't reach the store. Check the link and your connection, then try again.";
    }
}
