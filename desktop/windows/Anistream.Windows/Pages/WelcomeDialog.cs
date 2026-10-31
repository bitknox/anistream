using Anistream.App;
using Anistream.Engine.Ffi;
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
    readonly TextBox invite = new() { PlaceholderText = T.StoresInviteLinkHint, AcceptsReturn = false };
    readonly TextBlock problem = Ui.Text("", "HazeCaptionStyle");
    int page;

    /// <summary>An invite link to join once the dialog has closed: the join dialog takes it from there.</summary>
    public string? Invite { get; private set; }

    /// <summary>"folder" or "server" when the viewer chose to add their own media: that dialog opens once this closes.</summary>
    public string? Then { get; private set; }

    public WelcomeDialog(Ctx ctx, XamlRoot root)
    {
        this.ctx = ctx;
        AutomationProperties.SetName(invite, T.OnboardingInviteHint);
        invite.TextChanged += (_, _) => Check();
        dialog = new ContentDialog { XamlRoot = root, Content = body };
        dialog.PrimaryButtonClick += OnPrimary;
        dialog.SecondaryButtonClick += OnSecondary;
        Show(0);
    }

    public Task ShowAsync() => Dialogs.Show(dialog);

    /// <summary>Opens on a later page, for a screenshot: anilist or sources.</summary>
    public void Step(string name) => Show(name switch { "anilist" => 1, "sources" => 2, _ => 0 });

    /// <summary>An invite link opened while this is up: on to the sources page, filled in.</summary>
    public void Fill(string link)
    {
        invite.Text = link;
        Show(2);
    }

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
                var name = Ui.Text(T.AppName, "TitleTextBlockStyle");
                AutomationProperties.SetHeadingLevel(name, AutomationHeadingLevel.Level1);
                body.Children.Add(name);
                body.Children.Add(Ui.Text(T.OnboardingTagline, "BodyTextBlockStyle"));
                body.Children.Add(Ui.Haze(T.LegalHostsNoContent));
                dialog.PrimaryButtonText = T.OnboardingContinue;
                dialog.CloseButtonText = T.OnboardingSkip;
                break;
            case 1:
                var anilist = AnistreamFfiMethods.TrackerName("anilist");
                dialog.Title = anilist;
                body.Children.Add(Ui.Text(T.AccountsWelcomeDetail(anilist), "BodyTextBlockStyle"));
                body.Children.Add(Ui.Text(T.AccountsOtherTrackersNote, "HazeCaptionStyle"));
                dialog.PrimaryButtonText = T.AccountsSignInTo(anilist);
                dialog.SecondaryButtonText = T.OnboardingNotNow;
                break;
            default:
                dialog.Title = T.OnboardingWhereTitle;
                body.Children.Add(Ui.Text(T.LegalHostsNoContent, "BodyTextBlockStyle"));
                body.Children.Add(Ui.Haze(T.OnboardingWhereHosts));
                var own = Ui.Text(T.OnboardingOwnHeading, "BodyStrongTextBlockStyle");
                own.Margin = new Thickness(0, 8, 0, 0);
                AutomationProperties.SetHeadingLevel(own, AutomationHeadingLevel.Level2);
                body.Children.Add(own);
                var own2 = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
                own2.Children.Add(Ui.Label(T.SourcesAddFolderButton, (_, _) => Leave("folder")));
                own2.Children.Add(Ui.Label(T.SourcesAddServerButton, (_, _) => Leave("server")));
                body.Children.Add(own2);
                var link = Ui.Text(T.OnboardingInviteHeading, "BodyStrongTextBlockStyle");
                link.Margin = new Thickness(0, 8, 0, 0);
                AutomationProperties.SetHeadingLevel(link, AutomationHeadingLevel.Level2);
                body.Children.Add(link);
                body.Children.Add(invite);
                body.Children.Add(problem);
                body.Children.Add(Ui.Haze(T.PluginRiskWarning));
                dialog.PrimaryButtonText = T.OnboardingJoin;
                dialog.CloseButtonText = T.OnboardingSkipForNow;
                dialog.DefaultButton = ContentDialogButton.None;
                Check();
                break;
        }
    }

    void Leave(string then)
    {
        Then = then;
        dialog.Hide();
    }

    void Check()
    {
        if (page != 2)
            return;
        var link = Stores.InviteIn(invite.Text);
        dialog.IsPrimaryButtonEnabled = link is not null;
        problem.Text = invite.Text.Trim().Length > 0 && link is null ? T.StoresNotAnInvite : "";
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
