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
}
