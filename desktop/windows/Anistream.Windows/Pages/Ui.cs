using Anistream.Engine.Ffi;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Anistream.Windows.Pages;

/// <summary>Small pieces the pages built in code share: styled text, icon buttons, dots and rows.</summary>
static class Ui
{
    public static T Res<T>(string key) => (T)Application.Current.Resources[key];

    public static TextBlock Text(string text, string? style = null, bool wrap = true) => new()
    {
        Text = text,
        Style = style is null ? null : Res<Style>(style),
        TextWrapping = wrap ? TextWrapping.WrapWholeWords : TextWrapping.NoWrap,
        TextTrimming = wrap ? TextTrimming.None : TextTrimming.CharacterEllipsis,
    };

    public static TextBlock Haze(string text, bool wrap = true) => Text(text, "HazeBodyStyle", wrap);

    public static TextBlock Heading(string text) => new()
    {
        Text = text,
        Style = Res<Style>("BodyStrongTextBlockStyle"),
        Margin = new Thickness(0, 16, 0, 4),
    };

    /// <summary>A square icon button with a tooltip that is also its name.</summary>
    public static Button Icon(string glyph, string name, RoutedEventHandler click)
    {
        var button = new Button { Content = new FontIcon { Glyph = glyph, FontSize = 14 }, Width = 36, Height = 32, Padding = new Thickness(0) };
        ToolTipService.SetToolTip(button, name);
        AutomationProperties.SetName(button, name);
        button.Click += click;
        return button;
    }

    public static Button Label(string text, RoutedEventHandler click, bool accent = false)
    {
        var button = new Button { Content = text, Height = 32 };
        if (accent)
            button.Style = Res<Style>("AccentButtonStyle");
        button.Click += click;
        return button;
    }

    /// <summary>A status dot in a colour or, with <paramref name="ring"/>, an outline.</summary>
    public static Ellipse Dot(global::Windows.UI.Color colour, bool ring = false) => new()
    {
        Width = 8,
        Height = 8,
        VerticalAlignment = VerticalAlignment.Center,
        Fill = ring ? null : new SolidColorBrush(colour),
        Stroke = ring ? new SolidColorBrush(colour) : null,
        StrokeThickness = ring ? 1.5 : 0,
    };

    public static readonly global::Windows.UI.Color Vermilion = global::Windows.UI.Color.FromArgb(0xFF, 0xE0, 0x6B, 0x54);

    /// <summary>A flat block set off from the next by a hairline: hierarchy from spacing, not boxes.</summary>
    public static Border Card(UIElement child, double padding = 12) => new()
    {
        Child = child,
        Padding = new Thickness(0, padding, 0, padding),
        BorderBrush = Res<Brush>("DividerStrokeColorDefaultBrush"),
        BorderThickness = new Thickness(0, 0, 0, 1),
    };

    /// <summary>A cover thumbnail, in its placeholder colour until the engine's art arrives.</summary>
    public static Border Cover(Ctx ctx, uint id, double width, double height)
    {
        var frame = new Border
        {
            Width = width,
            Height = height,
            CornerRadius = new CornerRadius(6),
            Background = new SolidColorBrush(Colours.Raised),
        };
        if (ctx.Known.Entries.TryGetValue(id, out var entry))
        {
            frame.Background = new SolidColorBrush(Colours.Parse(entry.CoverColor) ?? Colours.Raised);
            ctx.Art.Bind(entry.CoverUrl, image => frame.Background = new ImageBrush { ImageSource = image, Stretch = Stretch.UniformToFill });
        }
        return frame;
    }

    public static string Bytes(ulong n) => AnistreamFfiMethods.HumanBytes(n);

    /// <summary>A destructive action's confirmation: Cancel is the default, the action names itself.</summary>
    public static async Task<bool> Confirm(XamlRoot root, string title, string text, string action)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = title,
            Content = Text(text, "BodyTextBlockStyle"),
            PrimaryButtonText = action,
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
        };
        return await dialog.ShowAsync() == ContentDialogResult.Primary;
    }
}
