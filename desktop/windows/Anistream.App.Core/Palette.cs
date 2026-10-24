using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>The Obi and Silk palette, dark, as the design brief gives it.</summary>
public static class Palette
{
    public const string Ground = "#161A2E";
    public const string Raised = "#222638";
    public const string Silk = "#E4E0D6";
    public const string Haze = "#9298B5";
    public const string Amber = "#F2A64B";
    public const string Water = "#7FC7D9";
    public const string Vermilion = "#E06B54";
    public const string Rule = "#434653";
    public const string ShowFont = "Zen Kaku Gothic New";

    /// <summary>How the engine dresses the player's own words: subtitles' style, its alerts.</summary>
    public static PlayerTheme PlayerTheme => new(
        Text: Silk, Dim: Haze, Obi: Amber, State: Water, Alert: Vermilion, Ground: Ground, Font: ShowFont);
}
