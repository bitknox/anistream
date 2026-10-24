using System.ComponentModel;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace Anistream.Windows;

/// <summary>
/// A title as a cover in a grid or a row in a list: its words from the model's own rules, its
/// placeholder in the cover's colour, and its art once the engine has fetched it.
/// </summary>
public sealed partial class CoverItem : INotifyPropertyChanged
{
    ImageSource? cover, banner;

    public CoverItem(Entry entry, string? caption = null)
    {
        Entry = entry;
        Caption = caption ?? AnistreamFfiMethods.CoverCaption(entry);
        Placeholder = new SolidColorBrush(Colours.Parse(entry.CoverColor) ?? Colours.Raised);
    }

    public Entry Entry { get; }
    public uint Id => Entry.Id;
    public string Title => Entry.Title;
    public string Caption { get; set; }
    public Brush Placeholder { get; }

    public ImageSource? Cover
    {
        get => cover;
        set { cover = value; PropertyChanged?.Invoke(this, new(nameof(Cover))); }
    }

    public ImageSource? Banner
    {
        get => banner;
        set { banner = value; PropertyChanged?.Invoke(this, new(nameof(Banner))); }
    }

    /// <summary>Ask for the cover, and the banner when <paramref name="withBanner"/>.</summary>
    internal CoverItem Load(Art art, bool withBanner = false)
    {
        art.Bind(Entry.CoverUrl, image => Cover = image);
        if (withBanner)
            art.Bind(Entry.BannerUrl ?? Entry.CoverUrl, image => Banner = image);
        return this;
    }

    public event PropertyChangedEventHandler? PropertyChanged;
}

static class Colours
{
    public static readonly Color Ground = Color.FromArgb(0xFF, 0x16, 0x1A, 0x2E);
    public static readonly Color Raised = Color.FromArgb(0xFF, 0x22, 0x26, 0x38);
    public static readonly Color Silk = Color.FromArgb(0xFF, 0xE4, 0xE0, 0xD6);
    public static readonly Color Haze = Color.FromArgb(0xFF, 0x92, 0x98, 0xB5);
    public static readonly Color Amber = Color.FromArgb(0xFF, 0xF2, 0xA6, 0x4B);
    public static readonly Color Water = Color.FromArgb(0xFF, 0x7F, 0xC7, 0xD9);

    /// <summary>"#rrggbb" as a colour, dimmed towards the ground so a placeholder stays quiet.</summary>
    public static Color? Parse(string? hex)
    {
        if (hex is not { Length: 7 } || hex[0] != '#' || !uint.TryParse(hex.AsSpan(1), System.Globalization.NumberStyles.HexNumber, null, out var rgb))
            return null;
        byte Mix(uint c, byte ground) => (byte)((c * 2 + ground * 3) / 5);
        return Color.FromArgb(0xFF, Mix((rgb >> 16) & 0xFF, Ground.R), Mix((rgb >> 8) & 0xFF, Ground.G), Mix(rgb & 0xFF, Ground.B));
    }

    public static SolidColorBrush Brush(Color c) => new(c);
}
