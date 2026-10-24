using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Anistream.Windows.Pages;

/// <summary>
/// Downloads: the queue by title, each title with its cover and its episodes, each episode with
/// what it is doing and what can be done to it; what is seeding after watching; and how much
/// room the downloads take of what the drive has.
/// </summary>
public sealed partial class DownloadsPage : Page
{
    readonly Ctx ctx = Ctx.Current;

    public DownloadsPage()
    {
        InitializeComponent();
        ctx.Batch += Heard;
        Unloaded += (_, _) => ctx.Batch -= Heard;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ctx.Send(new Command.LoadDownloads());
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.Downloads or Event.Seeding or Event.Settings or Event.Content))
            Show();
    }

    string? Folder => ctx.Known.Setting(SettingKey.DownloadDir) is { } row ? SettingsCatalog.TypedText(row) : null;

    ulong? Free()
    {
        try
        {
            var root = Path.GetPathRoot(string.IsNullOrEmpty(Folder) ? AppContext.BaseDirectory : Path.GetFullPath(Folder));
            return root is null ? null : (ulong)new DriveInfo(root).AvailableFreeSpace;
        }
        catch (Exception)
        {
            return null;
        }
    }

    void Show()
    {
        var queue = ctx.Known.Queue;
        var rows = queue.Rows;
        var used = queue.BytesOnDisk;
        var free = Free();
        Subtitle.Text = (free is { } f ? $"{Ui.Bytes(f)} free · " : "") + $"downloads use {Ui.Bytes(used)}";
        Meter.Visibility = free is null || used == 0 ? Visibility.Collapsed : Visibility.Visible;
        if (free is { } room)
            Meter.Value = Math.Min(1, (double)used / (used + room));
        PauseAll.IsEnabled = rows.Any(r => r.State is DownloadState.Active or DownloadState.Queued);
        ClearDone.IsEnabled = rows.Any(r => r.State == DownloadState.Done);

        Body.Children.Clear();
        if (rows.Length == 0 && ctx.Known.Seeding.Length == 0)
        {
            Body.Children.Add(Empty());
            return;
        }
        foreach (var group in queue.Groups())
            Body.Children.Add(Group(group));
        if (ctx.Known.Seeding.Length > 0)
        {
            Body.Children.Add(Ui.Heading("Seeding after watching"));
            foreach (var seed in ctx.Known.Seeding)
                Body.Children.Add(Seed(seed));
        }
        if (rows.Any(r => r.State is DownloadState.Active or DownloadState.Queued))
            Body.Children.Add(Ui.Haze(ctx.Window.WhenClosing switch
            {
                CloseChoice.KeepGoing => "Downloads keep going when you close the window.",
                CloseChoice.Stop => "Closing the window stops downloads. They resume the next time anistream opens.",
                _ => "Downloads keep going when you close the window, if you choose to.",
            }));
    }

    static StackPanel Empty() => States.Empty("No downloads", "Downloaded episodes play without a connection.");

    Border Group(DownloadGroup group)
    {
        var entry = ctx.Known.Entries.GetValueOrDefault(group.Id);
        var grid = new Grid { ColumnSpacing = 16 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var cover = Ui.Cover(ctx, group.Id, 64, 96);
        cover.VerticalAlignment = VerticalAlignment.Top;
        grid.Children.Add(cover);

        var column = new StackPanel { Spacing = 6 };
        Grid.SetColumn(column, 1);
        var head = new StackPanel { Spacing = 1 };
        head.Children.Add(Ui.Text(entry?.Title ?? group.Title, "BodyStrongTextBlockStyle", wrap: false));
        var bytes = group.BytesOnDisk;
        var count = AnistreamFfiMethods.Plural((ulong)group.Rows.Length, "episode");
        head.Children.Add(Ui.Text(bytes > 0 ? $"{count} · {Ui.Bytes(bytes)}" : count, "HazeCaptionStyle", wrap: false));
        column.Children.Add(head);
        foreach (var row in group.Rows)
            column.Children.Add(Episode(row));
        grid.Children.Add(column);
        return Ui.Card(grid);
    }

    Grid Episode(DownloadRow row)
    {
        var grid = new Grid { MinHeight = 40, ColumnSpacing = 8 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel { Spacing = 3, VerticalAlignment = VerticalAlignment.Center };
        var name = $"Episode {row.Episode}";
        var title = Ui.Text(name, wrap: false);
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        line.Children.Add(title);
        if (row.Ahead)
            line.Children.Add(Ui.Text("Downloaded ahead", "HazeCaptionStyle", wrap: false));
        text.Children.Add(line);
        text.Children.Add(State(row));
        if (row.State is DownloadState.Active or DownloadState.Paused && row.Total > 0)
        {
            var bar = new ProgressBar
            {
                Minimum = 0, Maximum = 1, Value = row.Fraction, Height = 4, MaxWidth = 420,
                HorizontalAlignment = HorizontalAlignment.Left, Width = 420,
                // Water while it runs, haze while it waits: a state, never the accent.
                Foreground = Ui.Res<Microsoft.UI.Xaml.Media.Brush>(row.State == DownloadState.Active ? "WaterBrush" : "HazeBrush"),
            };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(bar, $"{name} progress");
            text.Children.Add(bar);
        }
        grid.Children.Add(text);

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        switch (row.State)
        {
            case DownloadState.Done:
                actions.Children.Add(Ui.Icon("", $"Play {name}", (_, _) =>
                {
                    if (row.Path is { } path)
                        ctx.Send(new Command.PlayLocal(row.AnilistId, row.Episode, row.Title, path));
                }));
                actions.Children.Add(Ui.Icon("", "Show in folder", (_, _) => ShowInFolder(row.Path)));
                actions.Children.Add(Ui.Icon("", "Delete", (_, _) => ConfirmDelete(row, name)));
                break;
            case DownloadState.Active:
                actions.Children.Add(Ui.Icon("", "Pause", (_, _) => ctx.Send(new Command.DownloadPause(row.Id))));
                actions.Children.Add(Ui.Icon("", "Remove from queue", (_, _) => ctx.Send(new Command.DownloadCancel(row.Id))));
                break;
            case DownloadState.Paused:
                actions.Children.Add(Ui.Icon("", "Resume", (_, _) => ctx.Send(new Command.DownloadPause(row.Id))));
                actions.Children.Add(Ui.Icon("", "Remove from queue", (_, _) => ctx.Send(new Command.DownloadCancel(row.Id))));
                break;
            case DownloadState.Queued:
                actions.Children.Add(Ui.Icon("", "Remove from queue", (_, _) => ctx.Send(new Command.DownloadCancel(row.Id))));
                break;
            case DownloadState.Failed:
                actions.Children.Add(Ui.Label("Retry", (_, _) => ctx.Send(new Command.DownloadEpisode(row.AnilistId, row.Episode))));
                actions.Children.Add(Ui.Icon("", "Remove from queue", (_, _) => ctx.Send(new Command.DownloadCancel(row.Id))));
                break;
        }
        Grid.SetColumn(actions, 1);
        grid.Children.Add(actions);
        return grid;
    }

    UIElement State(DownloadRow row)
    {
        switch (row.State)
        {
            case DownloadState.Done:
            {
                var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
                line.Children.Add(new FontIcon { Glyph = "", FontSize = 12, Foreground = Ui.Res<Microsoft.UI.Xaml.Media.Brush>("WaterBrush") });
                line.Children.Add(Ui.Haze(row.Ahead ? "Downloaded · Removed a day after you watch it" : "Downloaded", wrap: false));
                return line;
            }
            case DownloadState.Active when row.Total == 0:
                return Ui.Haze("Waiting for metadata", wrap: false);
            case DownloadState.Active:
            {
                var text = $"{Ui.Bytes(row.Downloaded)} of {Ui.Bytes(row.Total)}";
                if (ctx.Known.Queue.Rates.TryGetValue(row.Id, out var rate) && rate > 0)
                    text += $" · {Ui.Bytes((ulong)rate)}/s";
                if (ctx.Known.Queue.SecondsLeft(row) is { } left)
                    text += $" · {AnistreamFfiMethods.Countdown(left)} left";
                return Ui.Haze(text, wrap: false);
            }
            case DownloadState.Paused:
                return Ui.Haze($"Paused at {Ui.Bytes(row.Downloaded)} of {Ui.Bytes(row.Total)}", wrap: false);
            case DownloadState.Queued:
                return Ui.Haze("Queued · starts when a download slot is free", wrap: false);
            default:
            {
                // The glyph is the one vermilion thing; the words are haze, and the engine's
                // reason goes to the log, not the row.
                var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
                line.Children.Add(States.ErrorGlyph(12));
                line.Children.Add(Ui.Haze("Couldn't finish downloading. Retry, or remove it from the queue.", wrap: false));
                if (row.Error is { } error)
                    ToolTipService.SetToolTip(line, Words.Failure(error));
                return line;
            }
        }
    }

    Border Seed(SeedRow seed)
    {
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel { Spacing = 3 };
        text.Children.Add(Ui.Text(seed.Name, wrap: false));
        text.Children.Add(Ui.Text(
            $"{seed.Ratio:0.0} of {seed.Goal:0.0} · {AnistreamFfiMethods.Countdown((long)seed.LeftSecs)} left · {Ui.Bytes(seed.UploadedBytes)} up",
            "HazeCaptionStyle", wrap: false));
        text.Children.Add(new ProgressBar
        {
            Minimum = 0, Maximum = 1, Height = 4, Width = 420, HorizontalAlignment = HorizontalAlignment.Left,
            Value = seed.Goal > 0 ? Math.Min(1, seed.Ratio / seed.Goal) : 0,
        });
        grid.Children.Add(text);
        if (seed.Size() is { } size)
        {
            var label = Ui.Haze(Ui.Bytes(size), wrap: false);
            label.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetColumn(label, 1);
            grid.Children.Add(label);
        }
        return grid.WrapCard();
    }

    async void ConfirmDelete(DownloadRow row, string name)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = $"Delete {name}?",
            Content = "The file is deleted from this computer.",
            PrimaryButtonText = "Delete",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
            ctx.Send(new Command.DownloadDelete(row.Id));
    }

    static async void ShowInFolder(string? path)
    {
        if (path is null)
            return;
        try
        {
            var folder = Path.GetDirectoryName(path);
            if (folder is not null)
                await global::Windows.System.Launcher.LaunchFolderPathAsync(folder);
        }
        catch (Exception)
        {
            // No folder to show: nothing to do.
        }
    }

    void OnPauseAll(object sender, RoutedEventArgs e) => ctx.PauseAllDownloads();

    void OnClearCompleted(object sender, RoutedEventArgs e) => ctx.Send(new Command.DownloadClearCompleted());

    async void OnOpenFolder(object sender, RoutedEventArgs e)
    {
        try
        {
            if (Folder is { Length: > 0 } folder && Directory.Exists(folder))
                await global::Windows.System.Launcher.LaunchFolderPathAsync(folder);
        }
        catch (Exception)
        {
            // No folder yet: nothing to open.
        }
    }
}

static class CardExtensions
{
    public static Border WrapCard(this UIElement child) => Ui.Card(child);
}
