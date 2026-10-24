using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows.Pages;

/// <summary>
/// First launch, as the spec has it everywhere: three short pages (Welcome, AniList, Sources),
/// each skippable, then the app on Continue. It says plainly that anistream hosts nothing and
/// that sources belong to other people, and never mentions torrents. Every step is in Settings
/// afterwards. On Windows the pages are one ContentDialog whose content and buttons change.
/// </summary>
sealed class WelcomeDialog
{
    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly StackPanel body = new() { Spacing = 12, MinWidth = 440, MaxWidth = 480 };
    readonly TextBox invite = new() { PlaceholderText = "https://…/join/…", AcceptsReturn = false };
    readonly TextBlock problem = Ui.Text("", "HazeCaptionStyle");
    int page;

    /// <summary>An invite link to join once the dialog has closed: the join dialog takes it from there.</summary>
    public string? Invite { get; private set; }

    public WelcomeDialog(Ctx ctx, XamlRoot root)
    {
        this.ctx = ctx;
        AutomationProperties.SetName(invite, "Invite link");
        invite.TextChanged += (_, _) => Check();
        dialog = new ContentDialog { XamlRoot = root, Content = body };
        dialog.PrimaryButtonClick += OnPrimary;
        dialog.SecondaryButtonClick += OnSecondary;
        Show(0);
    }

    public Task ShowAsync() => dialog.ShowAsync().AsTask();

    /// <summary>Opens on a later page, for a screenshot: anilist or sources.</summary>
    public void Step(string name) => Show(name switch { "anilist" => 1, "sources" => 2, _ => 0 });

    void Show(int to)
    {
        page = to;
        body.Children.Clear();
        dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        dialog.SecondaryButtonText = "";
        dialog.CloseButtonText = "";
        dialog.IsPrimaryButtonEnabled = true;
        dialog.DefaultButton = ContentDialogButton.Primary;
        switch (page)
        {
            case 0:
                dialog.Title = null;
                var nagare = new Nagare(5) { HorizontalAlignment = HorizontalAlignment.Left, Margin = new Thickness(0, 4, 0, 4) };
                AutomationProperties.SetAccessibilityView(nagare, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
                body.Children.Add(nagare);
                var name = Ui.Text("anistream", "TitleTextBlockStyle");
                AutomationProperties.SetHeadingLevel(name, AutomationHeadingLevel.Level1);
                body.Children.Add(name);
                body.Children.Add(Ui.Text("Keep track of anime and play it from sources you choose.", "BodyTextBlockStyle"));
                body.Children.Add(Ui.Haze("anistream hosts no content. Every source is run by someone else."));
                dialog.PrimaryButtonText = "Continue";
                dialog.CloseButtonText = "Skip";
                break;
            case 1:
                dialog.Title = "AniList";
                body.Children.Add(Ui.Text("Sign in to AniList to sync your list and where you stopped.", "BodyTextBlockStyle"));
                body.Children.Add(Ui.Text("Other trackers are in Settings › Accounts.", "HazeCaptionStyle"));
                dialog.PrimaryButtonText = "Sign in to AniList";
                dialog.SecondaryButtonText = "Not now";
                break;
            default:
                dialog.Title = "Sources";
                body.Children.Add(Ui.Text("anistream ships no sources of its own. Sources are plugins, shared through plugin stores by invite.", "BodyTextBlockStyle"));
                body.Children.Add(invite);
                body.Children.Add(problem);
                body.Children.Add(Ui.Haze("Each plugin is someone else's code connecting to someone else's service. Use them at your own risk."));
                dialog.PrimaryButtonText = "Join";
                dialog.CloseButtonText = "Skip for now";
                dialog.DefaultButton = ContentDialogButton.Primary;
                Check();
                break;
        }
    }

    void Check()
    {
        if (page != 2)
            return;
        var link = Stores.InviteIn(invite.Text);
        dialog.IsPrimaryButtonEnabled = link is not null;
        problem.Text = invite.Text.Trim().Length > 0 && link is null ? "That is not an invite link." : "";
        problem.Visibility = problem.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        switch (page)
        {
            case 0:
                args.Cancel = true;
                Show(1);
                break;
            case 1:
                args.Cancel = true;
                ctx.Send(new Command.Connect("anilist"));
                Show(2);
                break;
            default:
                // The join dialog shows the store before anything is joined; it opens once this closes.
                Invite = Stores.InviteIn(invite.Text);
                break;
        }
    }

    void OnSecondary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        if (page == 1)
        {
            args.Cancel = true;
            Show(2);
        }
    }
}
