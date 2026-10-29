using System.Diagnostics;
using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.Win32;

namespace Anistream.Windows.Player;

/// <summary>
/// Stats for nerds (player-stats.md): the engine's rows over the picture, with Copy stats and a
/// close button. The engine words every row and picks the one amber value; this lays them out.
/// I, the close button and Settings' Stats for nerds open and shut it; it stays up while the
/// controls hide and through the window's later playbacks, and hides in the mini player.
/// </summary>
public sealed partial class PlayerView
{
    /// <summary>The rows' type: a step under Body, so the usual values keep to one line whole.</summary>
    const double StatsFontSize = 13;
    const double StatsLineHeight = 20;
    /// <summary>Below this, the panel takes the player's width less the margins, as a phone's does.</summary>
    const double StatsNarrow = 560;
    /// <summary>Under the top bar's 56, its buttons and the menus that hang from them.</summary>
    const double StatsTop = 64;

    static readonly Stopwatch statsClock = Stopwatch.StartNew();
    readonly StatsPanel stats = new(() => statsClock.Elapsed);
    /// <summary>Each row's label and value, kept so a second's new figures change text in place.</summary>
    readonly List<(TextBlock Label, TextBlock Value)> statsLines = [];
    ToggleMenuFlyoutItem? statsItem;
    /// <summary>The panel is shown or fading in, so a tick that finds it the same leaves its fade alone.</summary>
    bool statsUp;

    void OnKeyStats(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, ToggleStats, show: false);

    void OnStatsClose(object sender, RoutedEventArgs e) => ToggleStats();

    void OnCopyStats(object sender, RoutedEventArgs e) => Send(new PlayerCommand.CopyStats(About()));

    /// <summary>Stats for nerds up or away, for the rest of this window's playbacks.</summary>
    void ToggleStats()
    {
        var hadFocus = StatsHasFocus();
        var told = stats.Toggle();
        if (statsItem is not null)
            statsItem.IsChecked = stats.Shown;
        Send(told);
        ShowStats();
        // The close button goes with the panel: the keys stay with the player.
        if (hadFocus)
            Focus(FocusState.Programmatic);
    }

    /// <summary>A new file is opening: the engine's player starts with its stats off, so ask again.</summary>
    void StatsReopen()
    {
        if (!stats.Shown)
            return;
        stats.Restart();
        // The player's own doing, not the viewer's: an episode that rolled on still counts.
        SendQuietly(new PlayerCommand.Stats(true));
        ShowStats();
    }

    void TookStats(PlayerStats now)
    {
        if (stats.Took(now))
            ShowStats();
    }

    /// <summary>The engine's text for the clipboard: Copy stats' lines.</summary>
    void TookClipboard(string text)
    {
        try
        {
            var package = new global::Windows.ApplicationModel.DataTransfer.DataPackage();
            package.SetText(text);
            global::Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(package);
        }
        catch (Exception e)
        {
            AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the clipboard refused the stats: {e.Message}");
            Alert(T.PlayerStatsCopyFailed);
            return;
        }
        if (stats.Copied())
            ShowStats();
    }

    bool StatsHasFocus()
    {
        if (XamlRoot is null || FocusManager.GetFocusedElement(XamlRoot) is not DependencyObject focused)
            return false;
        for (DependencyObject? at = focused; at is not null; at = VisualTreeHelper.GetParent(at))
        {
            if (at == StatsLayer)
                return true;
        }
        return false;
    }

    /// <summary>The panel as its states have it, faded in and out (fade-in, fade-out); values change in place.</summary>
    void ShowStats()
    {
        if (stats.View() is not { } view)
        {
            if (!statsUp)
                return;
            statsUp = false;
            if (StatsHasFocus())
                Focus(FocusState.Programmatic);
            Motion.Hide(StatsLayer);
            return;
        }
        StatsWaiting.Visibility = view.WaitingLine ? Visibility.Visible : Visibility.Collapsed;
        StatsCopy.IsEnabled = view.CanCopy;
        if (view.Copied && StatsCopy.FocusState != FocusState.Unfocused)
            Focus(FocusState.Programmatic);
        StatsCopy.Visibility = view.Copied ? Visibility.Collapsed : Visibility.Visible;
        StatsCopied.Visibility = view.Copied ? Visibility.Visible : Visibility.Collapsed;
        FillStats(view.Rows);
        PlaceStats();
        if (!statsUp)
        {
            statsUp = true;
            Motion.Show(StatsLayer);
        }
    }

    /// <summary>The rows, changed in place when they are the same rows as before.</summary>
    void FillStats(DetailRow[] rows)
    {
        var same = statsLines.Count == rows.Length && statsLines.Zip(rows).All(p => p.First.Label.Text == p.Second.Label);
        if (!same)
        {
            StatsRows.Children.Clear();
            StatsRows.RowDefinitions.Clear();
            statsLines.Clear();
            for (var i = 0; i < rows.Length; i++)
            {
                StatsRows.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
                var label = new TextBlock
                {
                    Text = rows[i].Label,
                    FontSize = StatsFontSize,
                    LineHeight = StatsLineHeight,
                    Foreground = (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"],
                    TextWrapping = TextWrapping.NoWrap,
                };
                var value = new TextBlock
                {
                    FontSize = StatsFontSize,
                    LineHeight = StatsLineHeight,
                    TextWrapping = TextWrapping.NoWrap,
                    TextTrimming = TextTrimming.CharacterEllipsis,
                };
                Microsoft.UI.Xaml.Documents.Typography.SetNumeralAlignment(value, FontNumeralAlignment.Tabular);
                // A value cut short on a narrow window: the whole of it on hover.
                value.IsTextTrimmedChanged += (v, _) => ToolTipService.SetToolTip(v, v.IsTextTrimmed ? v.Text : null);
                Grid.SetRow(label, i);
                Grid.SetRow(value, i);
                Grid.SetColumn(value, 1);
                StatsRows.Children.Add(label);
                StatsRows.Children.Add(value);
                statsLines.Add((label, value));
            }
        }
        var silk = (Brush)Application.Current.Resources["TextFillColorPrimaryBrush"];
        var amber = (Brush)Application.Current.Resources["AmberBrush"];
        foreach (var ((label, value), row) in statsLines.Zip(rows))
        {
            if (value.Text != row.Value)
            {
                value.Text = row.Value;
                AutomationProperties.SetName(value, T.PlayerStatsRow(row.Label, row.Value));
                if (value.IsTextTrimmed)
                    ToolTipService.SetToolTip(value, row.Value);
            }
            var brush = row.Warn ? amber : silk;
            if (value.Foreground != brush)
                value.Foreground = brush;
        }
    }

    /// <summary>
    /// Top trailing, 400 wide; the full width less 12 on a narrow window. It ends above the
    /// subtitles' band: two lines at mpv's size for the player's height, or the bottom bar,
    /// whichever is taller, so its rows scroll before it would cover a line. The band does not
    /// follow the controls, so the panel never moves as they come and go.
    /// </summary>
    void PlaceStats()
    {
        var width = Root.ActualWidth;
        var height = Root.ActualHeight;
        if (width <= 0 || height <= 0)
            return;
        var narrow = width < StatsNarrow;
        StatsLayer.HorizontalAlignment = narrow ? HorizontalAlignment.Stretch : HorizontalAlignment.Right;
        StatsLayer.Width = narrow ? double.NaN : 400;
        var side = narrow ? 12 : 16;
        StatsLayer.Margin = new Thickness(narrow ? side : 0, StatsTop, side, 0);
        // mpv's subtitles: 38 px a line at 720 high with its 22 px margin, scaled to the height.
        var band = Math.Max(BottomBar.ActualHeight, height * 0.17);
        StatsLayer.MaxHeight = Math.Max(120, height - StatsTop - band - 16);
    }

    void OnStatsSized(object sender, SizeChangedEventArgs e)
    {
        PlaceDiscussion();
        if (StatsLayer.Visibility == Visibility.Visible)
            PlaceStats();
    }

    /// <summary>Settings' Stats for nerds, a check that follows I and the close button.</summary>
    ToggleMenuFlyoutItem StatsItem()
    {
        statsItem = new ToggleMenuFlyoutItem { Text = T.PlayerStats, IsChecked = stats.Shown, KeyboardAcceleratorTextOverride = "I" };
        statsItem.Click += (_, _) =>
        {
            if (statsItem.IsChecked != stats.Shown)
                ToggleStats();
        };
        return statsItem;
    }

    /// <summary>This app, system and device, for Copy stats: nothing that names the viewer.</summary>
    static StatsAbout About()
    {
        var build = Environment.OSVersion.Version.Build;
        int revision = 0;
        string? release = null, product = null, version = null;
        try
        {
            using (var current = Registry.LocalMachine.OpenSubKey(@"SOFTWARE\Microsoft\Windows NT\CurrentVersion"))
            {
                revision = current?.GetValue("UBR") is int ubr ? ubr : 0;
                release = current?.GetValue("DisplayVersion") as string;
            }
            using var bios = Registry.LocalMachine.OpenSubKey(@"HARDWARE\DESCRIPTION\System\BIOS");
            product = bios?.GetValue("SystemProductName") as string;
            version = bios?.GetValue("SystemVersion") as string;
        }
        catch (Exception)
        {
            // Without the registry: the build alone, and no model.
        }
        return new StatsAbout(Updater.Current, StatsAbouts.System(build, revision, release), StatsAbouts.Device(product, version));
    }

#if ANISTREAM_TESTING
    /// <summary>
    /// For a shot: Stats for nerds up with the engine's <paramref name="fixture"/> held in place
    /// of its figures, <paramref name="waited"/> before the file opened, "Copied" up when
    /// <paramref name="copied"/>; the live panel when there is no fixture.
    /// </summary>
    void StageStats(StatsFixture? fixture, TimeSpan? waited, bool copied)
    {
        if (!stats.Shown)
            ToggleStats();
        if (fixture is { } f)
            stats.Stage(AnistreamFfiMethods.PlayerStatsFixture(f), waited, copied);
        ShowStats();
    }
#endif
}
