using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Shapes;

namespace Anistream.Windows.Pages;

/// <summary>
/// Turning torrents on, the same dialog asking for a proxy once they are on, and Edit proxy on
/// the saved one: the torrent index, what protects the connection with a proxy's address, and
/// how the connection is checked, said before anything starts. No protection asks to be
/// confirmed, and Cancel there comes back to this dialog with what was chosen. A proxy is
/// checked before anything is saved: the form locks while the check runs, only a passed check
/// saves it, and a failed one says why and saves nothing. The Mac's TorrentSheet and Linux's
/// <c>pages::torrents::setup</c>, in the same words, and Android's Edit proxy.
/// </summary>
sealed class TorrentDialog
{
    /// <summary>What the dialog is open for.</summary>
    public enum Purpose
    {
        /// <summary>Torrents turning on: the index, then what protects them.</summary>
        TurnOn,
        /// <summary>What protects them, changing once they are on.</summary>
        Protect,
        /// <summary>The saved proxy's details, filled in but for its password.</summary>
        EditProxy,
    }

    readonly Ctx ctx;
    readonly Purpose purpose;
    readonly ContentDialog dialog;
    readonly TextBox index = new() { Header = T.TorrentsIndex, PlaceholderText = T.TorrentsIndexHint };
    readonly StackPanel protections = new() { Spacing = 4, XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled };
    readonly List<RadioButton> choices = [];
    readonly TextBox host = new() { Header = T.TorrentsProxyHost, PlaceholderText = T.TorrentsProxyHostExample };
    readonly TextBox port = new() { Header = T.TorrentsProxyPort, PlaceholderText = "1080", Width = 96 };
    readonly TextBox user = new() { Header = T.TorrentsProxyUser };
    readonly PasswordBox password = new() { Header = T.TorrentsProxyPassword };
    readonly Grid proxy = new() { ColumnSpacing = 12, RowSpacing = 10, Margin = new Thickness(28, 4, 0, 8) };
    // The note and the checking or failed line share one place, so the dialog keeps its height
    // and its buttons stay where they are from the press to the verdict.
    readonly TextBlock note;
    readonly Grid line = new() { ColumnSpacing = 8, VerticalAlignment = VerticalAlignment.Top, Opacity = 0 };
    readonly ProgressRing ring = new() { Width = 14, Height = 14, IsActive = false, Opacity = 0, VerticalAlignment = VerticalAlignment.Top, Margin = new Thickness(0, 1, 0, 0) };
    readonly Ellipse dot = Ui.Dot(Ui.Vermilion);
    readonly TextBlock status = Ui.Text("", "CaptionTextBlockStyle");
    TorrentDraft draft;
    /// <summary>The draft the check under way was asked for; null while none runs.</summary>
    TorrentDraft? checking;
    /// <summary>What was saved, once it was: a passed check closes the dialog.</summary>
    TorrentProtection? saved;

    TorrentDialog(Ctx ctx, XamlRoot root, Purpose purpose, TorrentProtection? start, TorrentDraft? from)
    {
        this.ctx = ctx;
        this.purpose = purpose;
        var turningOn = purpose == Purpose.TurnOn;
        var editing = purpose == Purpose.EditProxy;
        draft = from ?? TorrentDraft.From(ctx.Known.Settings, turningOn, editing ? TorrentProtection.Proxy : start);

        var body = new StackPanel { Spacing = 12, Width = 460 };
        body.Children.Add(Ui.Text(editing ? TorrentWords.EditNote : TorrentWords.Exposed, "BodyTextBlockStyle"));
        if (turningOn)
        {
            index.Text = draft.Index;
            index.InputScope = new InputScope { Names = { new(InputScopeNameValue.Url) } };
            index.TextChanged += (_, _) => Changed(draft with { Index = index.Text });
            body.Children.Add(index);
            var indexNote = Ui.Text(TorrentWords.IndexNote, "HazeCaptionStyle");
            indexNote.Margin = new Thickness(0, -6, 0, 4);
            body.Children.Add(indexNote);
        }

        if (editing)
        {
            // Only the proxy: what protects torrents stays the proxy.
            proxy.Margin = new Thickness(0, 4, 0, 0);
            body.Children.Add(proxy);
        }
        else
        {
            // Protect with, the proxy's address under its choice, only while it is the choice:
            // the Mac's sheet. Buttons in one group, not RadioButtons, so the address sits
            // between them.
            protections.Children.Add(new TextBlock { Text = T.TorrentsProtectWith, Margin = new Thickness(0, 0, 0, 4) });
            body.Children.Add(protections);
            AutomationProperties.SetName(protections, T.TorrentsProtectWith);
            foreach (var choice in TorrentWords.Order)
            {
                var words = new StackPanel { Spacing = 2 };
                words.Children.Add(Ui.Text(TorrentWords.Label(choice)));
                words.Children.Add(Ui.Text(TorrentWords.Detail(choice), "HazeCaptionStyle"));
                var button = new RadioButton { Content = words, GroupName = "protect", IsChecked = choice == draft.Protection };
                AutomationProperties.SetName(button, T.TorrentsChoiceSpoken(TorrentWords.Label(choice), TorrentWords.Detail(choice)));
                button.Checked += (_, _) => Changed(draft with { Protection = choice });
                choices.Add(button);
                protections.Children.Add(button);
                if (choice == TorrentProtection.Proxy)
                    protections.Children.Add(proxy);
            }
        }

        proxy.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        proxy.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        proxy.RowDefinitions.Add(new RowDefinition());
        proxy.RowDefinitions.Add(new RowDefinition());
        host.Text = draft.Host;
        port.Text = draft.Port;
        user.Text = draft.User;
        port.InputScope = new InputScope { Names = { new(InputScopeNameValue.Number) } };
        host.TextChanged += (_, _) => Changed(draft with { Host = host.Text });
        port.TextChanged += (_, _) => Changed(draft with { Port = port.Text });
        user.TextChanged += (_, _) => Changed(draft with { User = user.Text });
        password.PasswordChanged += (_, _) => Changed(draft with { Password = password.Password });
        proxy.Children.Add(host);
        Grid.SetColumn(port, 1);
        proxy.Children.Add(port);
        Grid.SetRow(user, 1);
        proxy.Children.Add(user);
        Grid.SetRow(password, 1);
        Grid.SetColumn(password, 1);
        password.Width = 180;
        proxy.Children.Add(password);
        // An empty password keeps the saved one: said under the fields wherever one is saved. On
        // Edit proxy it is the note under the form instead.
        if (ctx.Known.Settings?.Torrent.Proxy is not null)
        {
            AutomationProperties.SetHelpText(password, TorrentWords.PasswordKept);
            if (!editing)
            {
                proxy.RowDefinitions.Add(new RowDefinition());
                var kept = Ui.Text(TorrentWords.PasswordKept, "HazeCaptionStyle");
                kept.Margin = new Thickness(0, -4, 0, 0);
                Grid.SetRow(kept, 2);
                Grid.SetColumnSpan(kept, 2);
                proxy.Children.Add(kept);
            }
        }

        // Under the form: what the check does, or on Edit proxy that an empty password keeps the
        // saved one, with the checking or failed line in its place once Save is pressed. Two
        // lines' room either way, so a failure that wraps moves nothing.
        note = Ui.Text(editing ? TorrentWords.PasswordKept : TorrentWords.CheckNote, "HazeCaptionStyle");
        Motion.QuickOpacity(note);
        var foot = new Grid { MinHeight = 32, Margin = new Thickness(0, 4, 0, 0) };
        foot.Children.Add(note);
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(14) });
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        line.Children.Add(ring);
        dot.VerticalAlignment = VerticalAlignment.Top;
        dot.HorizontalAlignment = HorizontalAlignment.Center;
        dot.Margin = new Thickness(0, 4, 0, 0);
        dot.Opacity = 0;
        line.Children.Add(dot);
        Grid.SetColumn(status, 1);
        line.Children.Add(status);
        Motion.QuickOpacity(line);
        AutomationProperties.SetAccessibilityView(line, AccessibilityView.Raw);
        foot.Children.Add(line);
        body.Children.Add(foot);

        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = purpose switch { Purpose.TurnOn => T.TorrentsDialogTurnOn, Purpose.Protect => T.TorrentsDialogProtect, _ => T.TorrentsDialogEditProxy },
            Content = body,
            PrimaryButtonText = turningOn ? T.TorrentsDialogTurnOnAction : T.ActionSave,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        dialog.PrimaryButtonClick += OnPrimary;
        // Cancel or Escape while the check runs: its verdict saves nothing.
        dialog.Closing += (_, _) =>
        {
            if (checking is not null)
                ctx.Trials.Drop();
            checking = null;
        };
        Changed(draft);
    }

    void Changed(TorrentDraft now)
    {
        // A detail changed after a failed check is a new try: the reason no longer stands.
        if (now != draft && checking is null && line.Opacity > 0)
            Say(null);
        draft = now;
        proxy.Visibility = draft.Protection == TorrentProtection.Proxy ? Visibility.Visible : Visibility.Collapsed;
        dialog.IsPrimaryButtonEnabled = draft.Complete && checking is null;
    }

    /// <summary>A proxy is checked first, the dialog staying up; the other choices close it.</summary>
    void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        if (draft.Protection != TorrentProtection.Proxy)
            return;
        args.Cancel = true;
        Check();
    }

    void Check()
    {
        if (checking is not null || !draft.Complete || draft.Protection != TorrentProtection.Proxy)
            return;
        checking = draft;
        Lock(true);
        Say(TorrentWords.CheckingProxy, busy: true);
        ctx.Trials.Ask(new TrialAsk(TorrentProtection.Proxy, draft.Proxy), (_, view) => Verdict(view));
    }

    /// <summary>The check's verdict, applied only to the draft it checked and only while the dialog waits for it.</summary>
    void Verdict(GuardView view)
    {
        if (checking is not { } asked || asked != draft)
            return;
        checking = null;
        if (view is GuardView.Protected)
        {
            foreach (var command in Torrents.Apply(asked, ctx.Known.Settings, purpose == Purpose.TurnOn))
                ctx.Send(command);
            saved = TorrentProtection.Proxy;
            dialog.Hide();
            return;
        }
        Lock(false);
        var kept = purpose switch
        {
            Purpose.TurnOn => TorrentWords.Kept.TurningOn,
            Purpose.Protect => TorrentWords.Kept.Protection,
            _ => TorrentWords.Kept.EditedProxy,
        };
        Say(TorrentWords.NotSaved(view, kept), error: true);
        host.Focus(FocusState.Programmatic);
    }

    /// <summary>The form while its check runs: nothing to change, and Save waits in its place.</summary>
    void Lock(bool locked)
    {
        index.IsEnabled = host.IsEnabled = port.IsEnabled = user.IsEnabled = password.IsEnabled = !locked;
        foreach (var choice in choices)
            choice.IsEnabled = !locked;
        dialog.IsPrimaryButtonEnabled = !locked && draft.Complete;
    }

    /// <summary>The line in the note's place: checking with its ring, failed with its dot, or null for the note.</summary>
    void Say(string? text, bool busy = false, bool error = false)
    {
        var saying = text is not null;
        status.Text = text ?? "";
        ring.IsActive = busy;
        ring.Opacity = busy ? 1 : 0;
        dot.Opacity = error ? 1 : 0;
        line.Opacity = saying ? 1 : 0;
        note.Opacity = saying ? 0 : 1;
        AutomationProperties.SetAccessibilityView(note, saying ? AccessibilityView.Raw : AccessibilityView.Content);
        AutomationProperties.SetAccessibilityView(line, saying ? AccessibilityView.Content : AccessibilityView.Raw);
        if (!saying)
            return;
        // Narrator says the check starting, and why nothing was saved at once.
        AutomationProperties.SetLiveSetting(status, error ? AutomationLiveSetting.Assertive : AutomationLiveSetting.Polite);
        FrameworkElementAutomationPeer.CreatePeerForElement(status)?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);
    }

#if ANISTREAM_TESTING
    /// <summary>A screenshot run's Save, once the dialog is up.</summary>
    public async void PressSave()
    {
        await Task.Delay(900);
        Check();
    }
#endif

    /// <summary>
    /// Shows the dialog, and sends what it chose once Turn on or Save is pressed: a proxy once
    /// its check passed, No protection once confirmed. What was saved, or null for Cancel, so a
    /// switch goes back off. <paramref name="start"/> is the choice it opens on in place of the
    /// one set, <paramref name="from"/> a whole draft to open on, and <paramref name="shown"/>
    /// is called once it is up.
    /// </summary>
    public static async Task<TorrentProtection?> Ask(Ctx ctx, XamlRoot root, Purpose purpose, TorrentProtection? start = null,
        TorrentDraft? from = null, Action<TorrentDialog>? shown = null)
    {
        var dialog = new TorrentDialog(ctx, root, purpose, start, from);
        var turningOn = purpose == Purpose.TurnOn;
        while (true)
        {
            var then = shown;
            shown = null;
            var result = await Dialogs.Show(dialog.dialog, then is null ? null : () => then(dialog));
            if (dialog.saved is { } done)
                return done;
            // A proxy is only ever saved by its check, above.
            if (result != ContentDialogResult.Primary || !dialog.draft.Complete || dialog.draft.Protection == TorrentProtection.Proxy)
                return null;
            // One dialog at a time: the confirmation takes this one's place, and Cancel there
            // brings it back with what was chosen.
            if (dialog.draft.Protection == TorrentProtection.None
                && !await Ui.Confirm(root, TorrentWords.NoneTitle, TorrentWords.NoneMessage, TorrentWords.NoneAction(turningOn)))
                continue;
            foreach (var command in Torrents.Apply(dialog.draft, ctx.Known.Settings, turningOn))
                ctx.Send(command);
            return dialog.draft.Protection;
        }
    }
}
