using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Anistream.Windows.Pages;

/// <summary>
/// Plugin stores: the ones this device has joined, and for the selected one what it offers (with
/// the hosts each plugin contacts) and what to do with it: install, update, remove, leave, open
/// its admin page. A store's owner also invites people and sees who has joined.
/// </summary>
public sealed partial class StoresPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    string? selected;
    bool quiet;
    string? inviteName;

    public StoresPage()
    {
        InitializeComponent();
        ctx.Batch += Heard;
        Unloaded += (_, _) => ctx.Batch -= Heard;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        selected = e.Parameter as string ?? ctx.Options.Select;
        ctx.Send(new Command.LoadStores());
        Show();
    }

    internal void Select(string store)
    {
        selected = store;
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.Stores or Event.StoreMembers or Event.InviteLink or Event.Providers))
            Show();
        foreach (var e in batch)
        {
            if (e is Event.StoreJoined joined)
                Select(joined.Store);
        }
    }

    void Show()
    {
        quiet = true;
        var stores = ctx.Known.Stores;
        var current = stores.FirstOrDefault(s => s.Id == selected) ?? stores.FirstOrDefault();
        selected = current?.Id;
        Empty.Visibility = stores.Length == 0 && ctx.Known.StoresLoaded ? Visibility.Visible : Visibility.Collapsed;
        StoreList.Items.Clear();
        foreach (var store in stores)
        {
            var item = new ListViewItem { Content = Line(store), Tag = store.Id };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(item, store.Name);
            StoreList.Items.Add(item);
            if (store.Id == current?.Id)
                StoreList.SelectedItem = item;
        }
        ShowDetail(current);
        quiet = false;
    }

    static Grid Line(StoreRow store)
    {
        var grid = new Grid { Height = 56, ColumnSpacing = 10 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(Ui.Text(store.Name, "BodyStrongTextBlockStyle", wrap: false));
        text.Children.Add(Ui.Text(store.Host, "HazeCaptionStyle", wrap: false));
        grid.Children.Add(text);
        var state = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
        Grid.SetColumn(state, 1);
        switch (store.State)
        {
            case StoreState.Offline:
                state.Children.Add(Ui.Dot(Colours.Haze, ring: true));
                state.Children.Add(Ui.Text("Offline", "HazeCaptionStyle", wrap: false));
                break;
            case StoreState.Revoked:
                state.Children.Add(Ui.Dot(Ui.Vermilion));
                state.Children.Add(Ui.Text("Access ended", "HazeCaptionStyle", wrap: false));
                break;
            default:
                if (Stores.Updates(store) is > 0 and var updates)
                {
                    state.Children.Add(Ui.Dot(Colours.Haze));
                    state.Children.Add(Ui.Text($"{updates} to update", "HazeCaptionStyle", wrap: false));
                }
                break;
        }
        grid.Children.Add(state);
        return grid;
    }

    void ShowDetail(StoreRow? store)
    {
        Detail.Children.Clear();
        if (store is null)
            return;
        Detail.Children.Add(Ui.Text(store.Name, "SubtitleTextBlockStyle"));
        Detail.Children.Add(Ui.Haze(Stores.RunBy(store.Owners, store.Host)));
        Detail.Children.Add(Ui.Text($"{Stores.JoinedAs(store)} · key {store.Fingerprint}", "HazeCaptionStyle"));
        switch (store.State)
        {
            case StoreState.Offline offline:
                Detail.Children.Add(Banner(InfoBarSeverity.Warning, "Not answering", Sentence(offline.V1) + " The plugins you installed keep working."));
                break;
            case StoreState.Revoked:
                Detail.Children.Add(Banner(InfoBarSeverity.Error, "Access ended", "This store's owner ended your access. Installed plugins keep working, and none will update."));
                break;
        }

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 4, 0, 0) };
        if (store.Owner)
            actions.Children.Add(Ui.Label("Open admin", (_, _) => ctx.Send(new Command.OpenAdmin(store.Id))));
        actions.Children.Add(Ui.Label("Leave store", (_, _) => ConfirmLeave(store)));
        Detail.Children.Add(actions);

        Detail.Children.Add(Ui.Heading("Plugins"));
        var plugins = store.Plugins ?? [];
        if (plugins.Length == 0)
            Detail.Children.Add(Ui.Haze("This store offers no plugins yet."));
        foreach (var plugin in plugins)
            Detail.Children.Add(Plugin(store, plugin));

        if (store.Owner)
            Owner(store);
    }

    static InfoBar Banner(InfoBarSeverity severity, string title, string text) =>
        new() { IsOpen = true, IsClosable = false, Severity = severity, Title = title, Message = text };

    static string Sentence(string text) => text.Length == 0 ? "" : char.ToUpperInvariant(text[0]) + text[1..] + (text.EndsWith('.') ? "" : ".");

    UIElement Plugin(StoreRow store, StorePluginRow plugin)
    {
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(Ui.Text(plugin.Name, "BodyStrongTextBlockStyle", wrap: false));
        var meta = $"Version {Stores.InstalledVersion(plugin)}";
        if (Stores.Standing(plugin) is { } standing)
            meta = plugin.Status is StorePluginStatus.Update u ? $"Version {u.From} installed, {plugin.Version} available" : $"{standing} · {meta}";
        text.Children.Add(Ui.Text(meta, "HazeCaptionStyle"));
        text.Children.Add(Ui.Text(Stores.ContactsLine(plugin), "HazeCaptionStyle"));
        if (plugin.Status is StorePluginStatus.Update { NewHosts: { Length: > 0 } added })
            text.Children.Add(Ui.Text("Newly contacts " + string.Join(", ", added), "HazeCaptionStyle"));
        grid.Children.Add(text);

        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        switch (plugin.Status)
        {
            case StorePluginStatus.Available:
                buttons.Children.Add(Ui.Label("Install", (_, _) => ctx.Send(new Command.InstallPlugin(store.Id, plugin.Id))));
                break;
            case StorePluginStatus.Update:
                buttons.Children.Add(Ui.Label("Update", (_, _) => ctx.Send(new Command.InstallPlugin(store.Id, plugin.Id))));
                buttons.Children.Add(Ui.Label("Remove", (_, _) => ctx.Send(new Command.RemovePlugin(plugin.Id))));
                break;
            case StorePluginStatus.Installed or StorePluginStatus.AddedByHand:
                buttons.Children.Add(Ui.Label("Remove", (_, _) => ctx.Send(new Command.RemovePlugin(plugin.Id))));
                break;
            case StorePluginStatus.OtherStore:
                break;
        }
        Grid.SetColumn(buttons, 1);
        grid.Children.Add(buttons);
        return Ui.Card(grid);
    }

    void Owner(StoreRow store)
    {
        Detail.Children.Add(Ui.Heading("People"));
        var members = ctx.Known.Members.GetValueOrDefault(store.Id);
        if (!ctx.Known.Members.ContainsKey(store.Id))
            ctx.Send(new Command.LoadMembers(store.Id));

        var invite = new Grid { ColumnSpacing = 8 };
        invite.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        invite.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var name = new TextBox { PlaceholderText = "Who is it for? (optional)", Text = inviteName ?? "" };
        name.TextChanged += (_, _) => inviteName = name.Text;
        invite.Children.Add(name);
        var make = Ui.Label("Make invite link", (_, _) => ctx.Send(new Command.CreateInvite(store.Id, string.IsNullOrWhiteSpace(inviteName) ? null : inviteName.Trim())));
        Grid.SetColumn(make, 1);
        invite.Children.Add(make);
        Detail.Children.Add(invite);

        if (ctx.Known.Invite is { } link && link.Store == store.Id)
        {
            var box = new TextBox { Text = link.Link, IsReadOnly = true };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, "Invite link");
            var row = new Grid { ColumnSpacing = 8 };
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.Children.Add(box);
            var copy = Ui.Label("Copy", (_, _) =>
            {
                var data = new global::Windows.ApplicationModel.DataTransfer.DataPackage();
                data.SetText(link.Link);
                global::Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(data);
            });
            Grid.SetColumn(copy, 1);
            row.Children.Add(copy);
            Detail.Children.Add(row);
            Detail.Children.Add(Ui.Text("Works once, for the person you send it to.", "HazeCaptionStyle"));
        }

        foreach (var member in members.Members ?? [])
        {
            var line = new Grid { ColumnSpacing = 8, Height = 36 };
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var label = member.Name + (member.You ? " (you)" : "") + (member.Owner ? " · owner" : "");
            var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
            text.Children.Add(Ui.Text(label, wrap: false));
            text.Children.Add(Ui.Text(AnistreamFfiMethods.Plural(member.Devices, "device"), "HazeCaptionStyle", wrap: false));
            line.Children.Add(text);
            if (!member.You && !member.Owner)
            {
                var remove = Ui.Label("Remove", (_, _) => ctx.Send(new Command.RemoveMember(store.Id, member.Id)));
                Grid.SetColumn(remove, 1);
                line.Children.Add(remove);
            }
            Detail.Children.Add(line);
        }
        foreach (var pending in members.Invites ?? [])
        {
            var line = new Grid { ColumnSpacing = 8, Height = 36 };
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            line.Children.Add(Ui.Haze($"Invite for {pending.Name}", wrap: false));
            var cancel = Ui.Label("Cancel invite", (_, _) => ctx.Send(new Command.CancelInvite(store.Id, pending.Id)));
            Grid.SetColumn(cancel, 1);
            line.Children.Add(cancel);
            Detail.Children.Add(line);
        }

        var open = new ToggleSwitch { IsOn = store.Open, OnContent = "Anyone with the open link can join", OffContent = "Only people you invite can join" };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(open, "Open link");
        open.Toggled += (_, _) =>
        {
            if (!quiet)
                ctx.Send(new Command.SetOpenJoin(store.Id, open.IsOn));
        };
        Detail.Children.Add(open);
    }

    async void ConfirmLeave(StoreRow store)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = $"Leave {store.Name}?",
            Content = "Its plugins stay installed but stop updating. You can join again with a new invite.",
            PrimaryButtonText = "Leave",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
            ctx.Send(new Command.LeaveStore(store.Id));
    }

    void OnSelected(object sender, SelectionChangedEventArgs e)
    {
        if (quiet || StoreList.SelectedItem is not ListViewItem { Tag: string id })
            return;
        selected = id;
        ShowDetail(ctx.Known.Stores.FirstOrDefault(s => s.Id == id));
    }

    async void OnJoin(object sender, RoutedEventArgs e) => await new JoinStoreDialog(ctx, XamlRoot).ShowAsync();
}
