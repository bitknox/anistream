using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows.Pages;

/// <summary>
/// About's update block: where the check and the update stand in one line, a bar while it
/// downloads, the release's notes, and the one step to take next: Check for updates, Cancel,
/// Restart to update or Try again. Each part keeps its place as the steps change.
/// </summary>
sealed class UpdatePanel : StackPanel
{
    readonly Ctx ctx;
    readonly Updater updater;
    readonly TextBlock line = new() { TextWrapping = TextWrapping.Wrap, MaxWidth = 520, HorizontalAlignment = HorizontalAlignment.Left };
    readonly ProgressBar bar = new() { Width = 320, Minimum = 0, Maximum = 1, HorizontalAlignment = HorizontalAlignment.Left, Margin = new Thickness(0, 2, 0, 2) };
    readonly TextBlock notes = new() { TextWrapping = TextWrapping.Wrap, MaxWidth = 520, HorizontalAlignment = HorizontalAlignment.Left, MaxLines = 4, TextTrimming = TextTrimming.WordEllipsis };
    readonly Button primary = new();
    readonly Button secondary = new();
    Action? onPrimary, onSecondary;
    string said = "";

    public UpdatePanel(Ctx ctx, Updater updater)
    {
        this.ctx = ctx;
        this.updater = updater;
        Spacing = 6;
        line.Style = Ui.Res<Style>("HazeBodyStyle");
        notes.Style = Ui.Res<Style>("HazeBodyStyle");
        notes.Opacity = 0.8;
        AutomationProperties.SetLiveSetting(line, AutomationLiveSetting.Polite);
        AutomationProperties.SetName(bar, "Update progress");
        primary.Click += (_, _) => onPrimary?.Invoke();
        secondary.Click += (_, _) => onSecondary?.Invoke();
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 6, 0, 0) };
        buttons.Children.Add(primary);
        buttons.Children.Add(secondary);
        Children.Add(line);
        Children.Add(bar);
        Children.Add(notes);
        Children.Add(buttons);
        Show();
        updater.Changed += Show;
        ctx.Batch += Heard;
        Unloaded += (_, _) =>
        {
            updater.Changed -= Show;
            ctx.Batch -= Heard;
        };
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.Update or Event.Connectivity))
            Show();
    }

    void Show()
    {
        var state = updater.State;
        var status = ctx.Known.Update;
        var release = updater.Release;
        var installer = updater.Installer;
        line.Text = Updates.Line(status, state, ctx.Known.Offline, installer is not null);
        if (line.Text != said)
        {
            said = line.Text;
            // Read out when the step changes, not with each megabyte.
            if (state.Step != UpdateStep.Downloading)
                FrameworkElementAutomationPeer.FromElement(line)?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);
        }

        var checking = status is UpdateStatus.Checking && state.Step == UpdateStep.Idle;
        bar.Opacity = state.Busy || checking ? 1 : 0;
        bar.IsIndeterminate = checking || state.Step is UpdateStep.Verifying or UpdateStep.Installing || state.Fraction is null;
        bar.Value = state.Fraction ?? 0;

        notes.Text = release?.Notes ?? "";
        notes.Visibility = release is { Notes.Length: > 0 } ? Visibility.Visible : Visibility.Collapsed;

        switch (state.Step)
        {
            case UpdateStep.Downloading or UpdateStep.Verifying:
                Buttons(("Cancel", updater.Stop, false), null);
                break;
            case UpdateStep.Ready:
                Buttons(("Restart to update", updater.Install, true), WhatsNew(release));
                break;
            case UpdateStep.Installing:
                Buttons(("Restart to update", null, true), null);
                break;
            case UpdateStep.Failed:
                Buttons(("Try again", updater.Download, false), WhatsNew(release));
                break;
            default:
                if (release is not null && installer is not null)
                    Buttons(("Download update", updater.Download, true), WhatsNew(release));
                else if (release is not null)
                    Buttons(("Open the release page", () => Open(release.PageUrl), true), Check(checking));
                else
                    Buttons(Check(checking), null);
                break;
        }
    }

    (string, Action?, bool) Check(bool checking) =>
        ("Check for updates", checking || ctx.Known.Offline ? null : () => ctx.Send(new Command.CheckForUpdates()), false);

    static (string, Action?, bool)? WhatsNew(Release? release) =>
        release is null ? null : ("What's new", () => Open(release.PageUrl), false);

    void Buttons((string Text, Action? Act, bool Accent) first, (string Text, Action? Act, bool Accent)? second)
    {
        Fill(primary, first, ref onPrimary);
        secondary.Visibility = second is null ? Visibility.Collapsed : Visibility.Visible;
        if (second is { } s)
            Fill(secondary, s, ref onSecondary);
    }

    static void Fill(Button button, (string Text, Action? Act, bool Accent) what, ref Action? on)
    {
        button.Content = what.Text;
        button.IsEnabled = what.Act is not null;
        button.Style = what.Accent ? Ui.Res<Style>("AccentButtonStyle") : null;
        on = what.Act;
    }

    static void Open(string url)
    {
        if (Uri.TryCreate(url, UriKind.Absolute, out var uri))
            _ = global::Windows.System.Launcher.LaunchUriAsync(uri);
    }
}
