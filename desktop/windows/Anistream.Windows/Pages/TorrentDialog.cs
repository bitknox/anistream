using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;

namespace Anistream.Windows.Pages;

/// <summary>
/// Turning torrents on, and the same dialog asking for a proxy once they are on: the torrent
/// index, what protects the connection with a proxy's address, and how the connection is
/// checked, said before anything starts. No protection asks to be confirmed, and Cancel there
/// comes back to this dialog with what was chosen. The Mac's TorrentSheet and Linux's
/// <c>pages::torrents::setup</c>, in the same words.
/// </summary>
sealed class TorrentDialog
{
    readonly Ctx ctx;
    readonly bool turningOn;
    readonly ContentDialog dialog;
    readonly TextBox index = new() { Header = "Torrent index", PlaceholderText = "https://…/?q={query}" };
    readonly StackPanel protections = new() { Spacing = 4, XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled };
    readonly TextBox host = new() { Header = "Host", PlaceholderText = "proxy.example" };
    readonly TextBox port = new() { Header = "Port", PlaceholderText = "1080", Width = 96 };
    readonly TextBox user = new() { Header = "User name" };
    readonly PasswordBox password = new() { Header = "Password" };
    readonly Grid proxy = new() { ColumnSpacing = 12, RowSpacing = 10, Margin = new Thickness(28, 4, 0, 8) };
    TorrentDraft draft;

    TorrentDialog(Ctx ctx, XamlRoot root, bool turningOn, TorrentProtection? start, TorrentDraft? from)
    {
        this.ctx = ctx;
        this.turningOn = turningOn;
        draft = from ?? TorrentDraft.From(ctx.Known.Settings, turningOn, start);

        var body = new StackPanel { Spacing = 12, Width = 460 };
        body.Children.Add(Ui.Text(TorrentWords.Exposed, "BodyTextBlockStyle"));
        if (turningOn)
        {
            index.Text = draft.Index;
            index.InputScope = new InputScope { Names = { new(InputScopeNameValue.Url) } };
            index.TextChanged += (_, _) => Changed(draft with { Index = index.Text });
            body.Children.Add(index);
            var note = Ui.Text(TorrentWords.IndexNote, "HazeCaptionStyle");
            note.Margin = new Thickness(0, -6, 0, 4);
            body.Children.Add(note);
        }

        // Protect with, the proxy's address under its choice, only while it is the choice: the
        // Mac's sheet. Buttons in one group, not RadioButtons, so the address sits between them.
        protections.Children.Add(new TextBlock { Text = "Protect with", Margin = new Thickness(0, 0, 0, 4) });
        body.Children.Add(protections);
        AutomationProperties.SetName(protections, "Protect with");
        foreach (var choice in TorrentWords.Order)
        {
            var words = new StackPanel { Spacing = 2 };
            words.Children.Add(Ui.Text(TorrentWords.Label(choice)));
            words.Children.Add(Ui.Text(TorrentWords.Detail(choice), "HazeCaptionStyle"));
            var button = new RadioButton { Content = words, GroupName = "protect", IsChecked = choice == draft.Protection };
            AutomationProperties.SetName(button, $"{TorrentWords.Label(choice)}. {TorrentWords.Detail(choice)}");
            button.Checked += (_, _) => Changed(draft with { Protection = choice });
            protections.Children.Add(button);
            if (choice == TorrentProtection.Proxy)
                protections.Children.Add(proxy);
        }

        proxy.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        proxy.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        proxy.RowDefinitions.Add(new RowDefinition());
        proxy.RowDefinitions.Add(new RowDefinition());
        host.Text = draft.Host;
        port.Text = draft.Port;
        user.Text = draft.User;
        port.InputScope = new InputScope { Names = { new(InputScopeNameValue.Number) } };
        if (ctx.Known.Settings?.Torrent.Proxy is not null)
            ToolTipService.SetToolTip(password, "Left empty, the password kept stays");
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

        var check = Ui.Text(TorrentWords.CheckNote, "HazeCaptionStyle");
        check.Margin = new Thickness(0, 4, 0, 0);
        body.Children.Add(check);

        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = turningOn ? "Turn on torrents" : "Protect torrents",
            Content = body,
            PrimaryButtonText = turningOn ? "Turn on" : "Save",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Primary,
        };
        dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        Changed(draft);
    }

    void Changed(TorrentDraft now)
    {
        draft = now;
        proxy.Visibility = draft.Protection == TorrentProtection.Proxy ? Visibility.Visible : Visibility.Collapsed;
        dialog.IsPrimaryButtonEnabled = draft.Complete;
    }

    /// <summary>
    /// Shows the dialog, and sends what it chose once Turn on or Save is pressed, after No
    /// protection is confirmed. Whether it sent: false for Cancel, so a switch goes back off.
    /// <paramref name="start"/> is the choice it opens on in place of the one set, and
    /// <paramref name="from"/> a whole draft to open on.
    /// </summary>
    public static async Task<bool> Ask(Ctx ctx, XamlRoot root, bool turningOn, TorrentProtection? start = null, TorrentDraft? from = null)
    {
        var shown = new TorrentDialog(ctx, root, turningOn, start, from);
        while (true)
        {
            if (await shown.dialog.ShowAsync() != ContentDialogResult.Primary || !shown.draft.Complete)
                return false;
            // One dialog at a time: the confirmation takes this one's place, and Cancel there
            // brings it back with what was chosen.
            if (shown.draft.Protection == TorrentProtection.None
                && !await Ui.Confirm(root, TorrentWords.NoneTitle, TorrentWords.NoneMessage, TorrentWords.NoneAction(shown.turningOn)))
                continue;
            foreach (var command in Torrents.Apply(shown.draft, ctx.Known.Settings, shown.turningOn))
                ctx.Send(command);
            return true;
        }
    }
}
