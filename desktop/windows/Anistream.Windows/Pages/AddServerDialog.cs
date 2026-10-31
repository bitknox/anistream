using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows.Pages;

/// <summary>
/// Adding a Jellyfin server in steps, as the board draws them: find it (on the network or by
/// address), sign in with a password or Quick Connect, choose libraries. Every error shows in
/// place, under the field or button that caused it, and the field keeps its text. The engine
/// answers each step with an event; this dialog only asks and draws.
/// </summary>
sealed class AddServerDialog
{
    enum Step { Find, SignIn, QuickConnect, Libraries }

    static readonly TimeSpan Silence = TimeSpan.FromMilliseconds(600), Looking = TimeSpan.FromSeconds(4);

    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly StackPanel body = new() { Spacing = 12, MinWidth = 520 };
    readonly TextBox address = new() { PlaceholderText = T.ServersAddressHint };
    readonly TextBox user = new();
    readonly PasswordBox password = new();
    readonly StackPanel found = new() { Spacing = 2 };
    readonly StackPanel libraryList = new() { Spacing = 2 };
    readonly ToggleSwitch report = new() { IsOn = true, OnContent = "", OffContent = "", Header = T.ServersReport };
    Step step = Step.Find;
    CheckedServer? checkedServer;
    string? signedIn;
    bool busy, discovered, noneFound;
    LibraryChoice[] libraries = [];
    CancellationTokenSource? wait;

    public AddServerDialog(Ctx ctx, XamlRoot root, string? again = null)
    {
        this.ctx = ctx;
        AutomationProperties.SetName(address, T.ServersAddress);
        AutomationProperties.SetName(user, T.ServersUser);
        AutomationProperties.SetName(password, T.ServersPassword);
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.ServersAddTitle,
            Content = new ScrollViewer { Content = body, MaxHeight = 560, VerticalScrollBarVisibility = ScrollBarVisibility.Auto },
            PrimaryButtonText = T.ServersContinue,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        dialog.PrimaryButtonClick += OnPrimary;
        address.TextChanged += (_, _) => { if (step == Step.Find) { Problem(null); Refresh(); } };
        address.KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.Enter && dialog.IsPrimaryButtonEnabled)
                dialog.Hide();
        };
        if (again is not null)
            address.Text = again;
        ShowFind();
    }

    public async Task ShowAsync()
    {
        ctx.Batch += Heard;
        ctx.Send(new Command.Server(new ServerCommand.Discover()));
        StartLooking();
        try
        {
            await Dialogs.Show(dialog);
        }
        finally
        {
            ctx.Batch -= Heard;
            wait?.Cancel();
            if (step == Step.QuickConnect)
                ctx.Send(new Command.Server(new ServerCommand.CancelQuickConnect()));
        }
    }

    // Draw.

    readonly TextBlock problemText = Ui.Text("", "BodyTextBlockStyle");
    readonly StackPanel problemLine = new() { Orientation = Orientation.Horizontal, Spacing = 8, Visibility = Visibility.Collapsed };

    /// <summary>An error line in place: a vermilion glyph and a sentence.</summary>
    void Problem(string? text)
    {
        if (problemLine.Children.Count == 0)
        {
            problemLine.Children.Add(new FontIcon { Glyph = "", FontSize = 14, Foreground = new Microsoft.UI.Xaml.Media.SolidColorBrush(Ui.Vermilion), VerticalAlignment = VerticalAlignment.Top, Margin = new Thickness(0, 3, 0, 0) });
            problemText.FontSize = 13;
            problemLine.Children.Add(problemText);
            AutomationProperties.SetLiveSetting(problemText, Microsoft.UI.Xaml.Automation.Peers.AutomationLiveSetting.Assertive);
        }
        problemText.Text = text ?? "";
        problemLine.Visibility = text is null ? Visibility.Collapsed : Visibility.Visible;
    }

    /// <summary>The one error line, moved to <paramref name="panel"/>.</summary>
    void Attach(Panel panel, int at = -1)
    {
        (problemLine.Parent as Panel)?.Children.Remove(problemLine);
        if (at < 0)
            panel.Children.Add(problemLine);
        else
            panel.Children.Insert(at, problemLine);
    }

    void Clear(string title, string primary)
    {
        body.Children.Clear();
        dialog.Title = title;
        dialog.PrimaryButtonText = primary;
        dialog.IsPrimaryButtonEnabled = true;
        Problem(null);
    }

    void Refresh()
    {
        if (step == Step.Find)
            dialog.IsPrimaryButtonEnabled = !busy && address.Text.Trim().Length > 0;
        else if (step == Step.SignIn)
            dialog.IsPrimaryButtonEnabled = !busy && user.Text.Trim().Length > 0;
    }

    void ShowFind()
    {
        step = Step.Find;
        Clear(T.ServersAddTitle, T.ServersContinue);
        body.Children.Add(Ui.Text(T.ServersOnNetwork, "BodyStrongTextBlockStyle"));
        body.Children.Add(found);
        RedrawFound();
        body.Children.Add(Ui.Text(T.ServersAddress, "BodyTextBlockStyle"));
        body.Children.Add(address);
        Attach(body);
        Refresh();
    }

    readonly List<FoundServer> servers = [];
    bool lookingShown;

    void RedrawFound()
    {
        found.Children.Clear();
        foreach (var server in servers)
        {
            var row = new Button
            {
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Left,
                Content = new TextBlock { Text = $"{server.Name} · {Display(server.Address)}" },
                Tag = server,
                Style = Ui.Res<Style>("SubtleButtonStyle"),
            };
            row.Click += (_, _) =>
            {
                address.Text = Display(server.Address);
                address.Focus(FocusState.Programmatic);
            };
            found.Children.Add(row);
        }
        if (!discovered && lookingShown && servers.Count == 0)
        {
            var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            line.Children.Add(new ProgressRing { Width = 16, Height = 16, IsActive = true });
            line.Children.Add(Ui.Haze(T.ServersLooking));
            found.Children.Add(line);
        }
        else if (noneFound && servers.Count == 0)
            found.Children.Add(Ui.Haze(T.ServersNoneFound));
    }

    static string Display(string address) =>
        address.StartsWith("http://", StringComparison.OrdinalIgnoreCase) ? address[7..] : address;

    /// <summary>The list stays silent for 600 ms, then shows that it is looking; after 4 s with nothing it says so.</summary>
    async void StartLooking()
    {
        wait = new CancellationTokenSource();
        var token = wait.Token;
        try
        {
            await Task.Delay(Silence, token);
            lookingShown = true;
            if (step == Step.Find)
                RedrawFound();
            await Task.Delay(Looking - Silence, token);
            if (servers.Count == 0)
            {
                noneFound = true;
                if (step == Step.Find)
                    RedrawFound();
            }
        }
        catch (TaskCanceledException)
        {
        }
    }

    void ShowSignIn()
    {
        step = Step.SignIn;
        var name = checkedServer?.Name ?? address.Text.Trim();
        Clear(T.ServersSignInTitle(name), T.ServersSignIn);
        body.Children.Add(Ui.Haze(checkedServer is { } c ? Display(c.Address) : address.Text.Trim()));
        body.Children.Add(Ui.Text(T.ServersUser, "BodyTextBlockStyle"));
        body.Children.Add(user);
        body.Children.Add(Ui.Text(T.ServersPassword, "BodyTextBlockStyle"));
        body.Children.Add(password);
        Attach(body);
        if (checkedServer?.QuickConnect == true)
        {
            var link = new HyperlinkButton { Content = T.ServersUseQuickConnect, Padding = new Thickness(0) };
            link.Click += (_, _) => StartQuickConnect();
            body.Children.Add(link);
        }
        body.Children.Add(Ui.Text(T.ServersTokenNote(T.SecureStoreWindows), "HazeCaptionStyle"));
        user.TextChanged -= OnUserChanged;
        user.TextChanged += OnUserChanged;
        Refresh();
        user.Focus(FocusState.Programmatic);
    }

    void OnUserChanged(object sender, TextChangedEventArgs e) => Refresh();

    readonly TextBlock code = Ui.Text("", "TitleLargeTextBlockStyle", wrap: false);
    readonly StackPanel waiting = new() { Orientation = Orientation.Horizontal, Spacing = 8, Visibility = Visibility.Collapsed };
    readonly StackPanel newCode = new() { Spacing = 8, Visibility = Visibility.Collapsed };

    void StartQuickConnect()
    {
        step = Step.QuickConnect;
        Clear(T.ServersQcTitle, T.ServersSignInAgain);
        dialog.PrimaryButtonText = "";
        dialog.IsPrimaryButtonEnabled = false;
        code.Text = "";
        code.CharacterSpacing = 120;
        body.Children.Add(code);
        body.Children.Add(Ui.Text(T.ServersQcSteps, "BodyTextBlockStyle"));
        if (waiting.Children.Count == 0)
        {
            waiting.Children.Add(new ProgressRing { Width = 16, Height = 16, IsActive = true });
            waiting.Children.Add(Ui.Haze(T.ServersQcWaiting));
            var again = Ui.Label(T.ServersQcNewCode, (_, _) => StartQuickConnect());
            again.HorizontalAlignment = HorizontalAlignment.Left;
            newCode.Children.Add(again);
        }
        waiting.Visibility = Visibility.Collapsed;
        newCode.Visibility = Visibility.Collapsed;
        Attach(newCode, 0);
        body.Children.Add(waiting);
        body.Children.Add(newCode);
        var password = Ui.Label(T.ServersUsePassword, (_, _) => { ctx.Send(new Command.Server(new ServerCommand.CancelQuickConnect())); ShowSignIn(); });
        password.HorizontalAlignment = HorizontalAlignment.Left;
        body.Children.Add(password);
        ctx.Send(new Command.Server(new ServerCommand.StartQuickConnect(checkedServer?.Address ?? address.Text.Trim())));
    }

    void ShowLibraries()
    {
        step = Step.Libraries;
        Clear(T.ServersLibrariesTitle, T.ServersAdd);
        body.Children.Add(Ui.Text(T.ServersLibrariesBody, "BodyTextBlockStyle"));
        libraryList.Children.Clear();
        foreach (var library in libraries)
        {
            var box = new CheckBox { IsChecked = library.Ticked, Tag = library.Id, Content = $"{library.Name}\n{Lawful.LibraryKind(library.Kind)}" };
            var line = new StackPanel { Spacing = 0 };
            line.Children.Add(Ui.Text(library.Name, wrap: false));
            line.Children.Add(Ui.Text(Lawful.LibraryKind(library.Kind), "HazeCaptionStyle", wrap: false));
            box.Content = line;
            AutomationProperties.SetName(box, $"{library.Name}, {Lawful.LibraryKind(library.Kind)}");
            box.Click += (_, _) => dialog.IsPrimaryButtonEnabled = libraryList.Children.OfType<CheckBox>().Any(b => b.IsChecked == true);
            libraryList.Children.Add(box);
        }
        body.Children.Add(libraryList);
        body.Children.Add(report);
        body.Children.Add(Ui.Haze(T.ServersReportLine));
        dialog.IsPrimaryButtonEnabled = libraries.Any(l => l.Ticked);
    }

    // Ask.

    void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        switch (step)
        {
            case Step.Find:
                args.Cancel = true;
                Check();
                break;
            case Step.SignIn:
                args.Cancel = true;
                SignIn();
                break;
            case Step.Libraries:
                var ticked = libraryList.Children.OfType<CheckBox>().Where(b => b.IsChecked == true).Select(b => (string)b.Tag).ToArray();
                if (signedIn is { } server)
                {
                    ctx.Send(new Command.Server(new ServerCommand.SetLibraries(server, ticked)));
                    if (!report.IsOn)
                        ctx.Send(new Command.Server(new ServerCommand.SetReport(server, false)));
                }
                ctx.Send(new Command.Server(new ServerCommand.Load()));
                break;
            default:
                args.Cancel = true;
                break;
        }
    }

    /// <summary>Busy: silent for 600 ms, then a spinner beside the button's words.</summary>
    async void Busy(bool on)
    {
        busy = on;
        Refresh();
        if (!on)
        {
            dialog.PrimaryButtonText = step switch { Step.SignIn => T.ServersSignIn, _ => T.ServersContinue };
            return;
        }
        await Task.Delay(Silence);
        if (busy)
            dialog.PrimaryButtonText = "…";
    }

    void Check()
    {
        Problem(null);
        Busy(true);
        ctx.Send(new Command.Server(new ServerCommand.Check(address.Text.Trim())));
    }

    void SignIn()
    {
        Problem(null);
        Busy(true);
        ctx.Send(new Command.Server(new ServerCommand.SignIn(checkedServer?.Address ?? address.Text.Trim(), user.Text.Trim(), password.Password)));
    }

    // Hear.

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            if (e is not Event.Server { V1: var server })
                continue;
            switch (server)
            {
                case ServerEvent.Found f when servers.All(s => s.Id != f.V1.Id):
                    servers.Add(f.V1);
                    if (step == Step.Find)
                        RedrawFound();
                    break;
                case ServerEvent.DiscoveryDone:
                    discovered = true;
                    noneFound = servers.Count == 0 && lookingShown;
                    if (step == Step.Find)
                        RedrawFound();
                    break;
                case ServerEvent.Checked c:
                    Busy(false);
                    checkedServer = c.V1;
                    ShowSignIn();
                    break;
                case ServerEvent.Failed { Step: var at, Failure: var failure }:
                    Busy(false);
                    Fail(at, failure);
                    break;
                case ServerEvent.QuickConnectCode q when step == Step.QuickConnect:
                    code.Text = q.Code;
                    waiting.Visibility = Visibility.Visible;
                    break;
                case ServerEvent.SignedIn s:
                    Busy(false);
                    signedIn = s.Server;
                    ctx.Send(new Command.Server(new ServerCommand.LoadLibraries(s.Server)));
                    break;
                case ServerEvent.Libraries l when l.Server == signedIn:
                    libraries = l.LibrariesValue;
                    ShowLibraries();
                    break;
            }
        }
    }

    void Fail(ServerStep at, ServerFailure failure)
    {
        var text = Lawful.Failure(failure, address.Text.Trim());
        switch (at)
        {
            case ServerStep.QuickConnect when failure is ServerFailure.QuickConnectExpired:
                waiting.Visibility = Visibility.Collapsed;
                newCode.Visibility = Visibility.Visible;
                Problem(text);
                break;
            case ServerStep.QuickConnect:
                // Off on this server: the error replaces the link, and the password is the way on.
                ShowSignIn();
                Problem(text);
                break;
            case ServerStep.SignIn:
                if (step != Step.SignIn)
                    ShowSignIn();
                Problem(text);
                break;
            default:
                if (step != Step.Find)
                    ShowFind();
                Problem(text);
                break;
        }
    }
}
