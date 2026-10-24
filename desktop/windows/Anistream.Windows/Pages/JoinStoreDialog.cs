using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows.Pages;

/// <summary>
/// Joining a plugin store in two steps, as the other apps do: paste the invite link and let the
/// engine check it, then see who runs the store, what it offers and which hosts those plugins
/// contact, and confirm. Nothing is joined or installed until the second step's Join.
/// </summary>
sealed class JoinStoreDialog
{
    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly TextBox invite = new() { PlaceholderText = "https://…/join/…", AcceptsReturn = false };
    readonly TextBlock problem = Ui.Text("", "HazeCaptionStyle");
    readonly StackPanel body = new() { Spacing = 12, MinWidth = 420 };
    InvitedStore? preview;
    TextBox? deviceName;

    public JoinStoreDialog(Ctx ctx, XamlRoot root, string? pasted = null)
    {
        this.ctx = ctx;
        AutomationProperties.SetName(invite, "Invite link");
        invite.Text = pasted ?? "";
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = "Add a plugin store",
            Content = body,
            PrimaryButtonText = "Check invite",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Primary,
        };
        dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        dialog.PrimaryButtonClick += OnPrimary;
        invite.TextChanged += (_, _) => Update();
        FirstStep();
    }

    /// <summary>Show it; with <paramref name="checkAtOnce"/> the invite is checked as soon as it is up, for a screenshot.</summary>
    public async Task ShowAsync(bool checkAtOnce = false)
    {
        if (checkAtOnce)
            _ = Task.Delay(700).ContinueWith(_ => dialog.DispatcherQueue.TryEnqueue(Check));
        ctx.Known.Preview = null;
        ctx.Known.Joined = null;
        ctx.Batch += Heard;
        try
        {
            await dialog.ShowAsync();
        }
        finally
        {
            ctx.Batch -= Heard;
        }
    }

    void FirstStep()
    {
        preview = null;
        body.Children.Clear();
        body.Children.Add(Ui.Text("Paste the invite link a plugin store's owner sent you. You see what the store offers before anything is joined or installed.", "BodyTextBlockStyle"));
        body.Children.Add(invite);
        body.Children.Add(problem);
        dialog.Title = "Add a plugin store";
        dialog.PrimaryButtonText = "Check invite";
        Update();
    }

    void Update()
    {
        if (preview is not null)
            return;
        dialog.IsPrimaryButtonEnabled = Stores.InviteIn(invite.Text) is not null;
        problem.Text = invite.Text.Trim().Length > 0 && !dialog.IsPrimaryButtonEnabled ? "That is not an invite link." : "";
    }

    void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        if (preview is null)
        {
            args.Cancel = true;
            Check();
            return;
        }
        var label = preview.Anonymous ? null : deviceName?.Text.Trim() is { Length: > 0 } name ? name : null;
        ctx.Send(new Command.JoinStore(preview.Link, label));
    }

    /// <summary>Step one: ask the engine what the link leads to, and stay until it answers.</summary>
    void Check()
    {
        if (preview is not null || Stores.InviteIn(invite.Text) is not { } link)
            return;
        problem.Text = "Checking the invite";
        dialog.IsPrimaryButtonEnabled = false;
        ctx.Send(new Command.PreviewStore(link));
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.StorePreview { V1: InvitePreview.Store s }:
                    SecondStep(s.V1);
                    break;
                case Event.StorePreview { V1: InvitePreview.Failed f }:
                    problem.Text = Words.InviteFailure(f.Reason);
                    dialog.IsPrimaryButtonEnabled = true;
                    break;
                case Event.StoreJoined:
                    dialog.Hide();
                    break;
            }
        }
    }

    void SecondStep(InvitedStore store)
    {
        preview = store;
        body.Children.Clear();
        dialog.Title = store.AlreadyJoined ? $"{store.Name} is already joined" : $"Join {store.Name}?";
        body.Children.Add(Ui.Haze(Stores.RunBy(store.Owners, store.Host)));
        body.Children.Add(Ui.Text($"Key {store.Fingerprint}", "HazeCaptionStyle"));
        if (store.Open)
            body.Children.Add(Ui.Haze("Anyone with the link can join this store."));
        var plugins = store.Plugins ?? [];
        if (plugins.Length > 0)
        {
            body.Children.Add(Ui.Text(AnistreamPlural(plugins.Length), "BodyStrongTextBlockStyle"));
            var list = new StackPanel { Spacing = 8 };
            foreach (var plugin in plugins)
            {
                var line = new StackPanel();
                line.Children.Add(Ui.Text($"{plugin.Name} {plugin.Version}", wrap: false));
                line.Children.Add(Ui.Text(Stores.ContactsLine(plugin), "HazeCaptionStyle", wrap: false));
                list.Children.Add(line);
            }
            body.Children.Add(new ScrollViewer { Content = list, MaxHeight = 200, VerticalScrollBarVisibility = ScrollBarVisibility.Auto });
        }
        body.Children.Add(Ui.Haze("Joining installs nothing. You choose which plugins to install, and each one asks for the hosts it contacts."));
        if (!store.Anonymous && !store.AlreadyJoined)
        {
            deviceName = new TextBox { Text = store.DeviceLabel };
            AutomationProperties.SetName(deviceName, "Name this device is shown under");
            body.Children.Add(Ui.Text("The owner sees this device as", "HazeCaptionStyle"));
            body.Children.Add(deviceName);
        }
        dialog.PrimaryButtonText = store.AlreadyJoined ? "Open store" : "Join store";
        dialog.IsPrimaryButtonEnabled = true;
        dialog.DefaultButton = ContentDialogButton.Primary;
    }

    static string AnistreamPlural(int n) => Anistream.Engine.Ffi.AnistreamFfiMethods.Plural((ulong)n, "plugin");
}
