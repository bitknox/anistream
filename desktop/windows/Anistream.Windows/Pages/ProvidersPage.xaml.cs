using Anistream.App;
using Anistream.Engine.Core;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Navigation;

namespace Anistream.Windows.Pages;

/// <summary>
/// Providers: the sources in the order failover tries them, moved by dragging or by Alt with the
/// up and down arrows, and the selected one's version, speed, what it may contact and what can be
/// done to it: update, switch off, remove.
/// </summary>
public sealed partial class ProvidersPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    string? selected;
    bool quiet;

    public ProvidersPage()
    {
        InitializeComponent();
        ctx.Batch += Heard;
        Unloaded += (_, _) => ctx.Batch -= Heard;
        Order.KeyDown += OnOrderKey;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        selected = ctx.Options.Select;
        ctx.Send(new Command.CheckProviders());
        if (!ctx.Known.StoresLoaded)
            ctx.Send(new Command.LoadStores());
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.Providers or Event.ProviderNote or Event.SourcesChanged or Event.Stores or Event.Settings))
            Show();
    }

    ProviderInfo Info(ProviderRow row) => ProviderInfo.Of(row, ctx.Known.Stores);

    void Show()
    {
        quiet = true;
        var rows = ctx.Known.OrderedProviders;
        Rebuilding.Visibility = ctx.Known.Rebuilding ? Visibility.Visible : Visibility.Collapsed;
        Note.Text = ctx.Known.ProviderNote ?? "";
        Note.Visibility = rows.Length == 0 && ctx.Known.ProviderNote is not null ? Visibility.Visible : Visibility.Collapsed;
        var current = rows.FirstOrDefault(r => r.Id == selected) ?? rows.FirstOrDefault();
        selected = current?.Id;
        Order.Items.Clear();
        foreach (var row in rows)
        {
            var item = new ListViewItem { Content = Line(Info(row), row.Id == current?.Id), Tag = row.Id };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(item, Info(row).Name);
            Order.Items.Add(item);
            if (row.Id == current?.Id)
                Order.SelectedItem = item;
        }
        ShowDetail(current is null ? null : Info(current));
        quiet = false;
    }

    static Grid Line(ProviderInfo info, bool selected)
    {
        var grid = new Grid { Height = 48, ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.Children.Add(new FontIcon { Glyph = "", FontSize = 12, VerticalAlignment = VerticalAlignment.Center, Foreground = Ui.Res<Microsoft.UI.Xaml.Media.Brush>("TextFillColorSecondaryBrush") });
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(Ui.Text(info.Name, "BodyStrongTextBlockStyle", wrap: false));
        text.Children.Add(Ui.Text(info.Origin, "HazeCaptionStyle", wrap: false));
        Grid.SetColumn(text, 1);
        grid.Children.Add(text);
        var standing = Standing(info);
        Grid.SetColumn(standing, 2);
        grid.Children.Add(standing);
        return grid;
    }

    /// <summary>How a source stands beside its name: failing, held back, slow, off, or an update waiting.</summary>
    static StackPanel Standing(ProviderInfo info)
    {
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
        var standing = info.Row.Standing();
        switch (standing)
        {
            case ProviderStanding.Failing:
                line.Children.Add(Ui.Dot(Ui.Vermilion));
                break;
            case ProviderStanding.HeldBack or ProviderStanding.Off:
                line.Children.Add(Ui.Dot(Colours.Haze, ring: true));
                break;
            case ProviderStanding.Slow:
                line.Children.Add(Ui.Dot(Colours.Haze));
                break;
        }
        if (standing.StandingLine() is { } words)
            line.Children.Add(Ui.Text(words, "HazeCaptionStyle", wrap: false));
        else if (info.Update is not null)
        {
            line.Children.Add(Ui.Dot(Colours.Haze));
            line.Children.Add(Ui.Text("Update", "HazeCaptionStyle", wrap: false));
        }
        else if (info.Row.LatencyMs is { } ms)
            line.Children.Add(Ui.Text($"{ms} ms", "HazeCaptionStyle", wrap: false));
        return line;
    }

    void ShowDetail(ProviderInfo? info)
    {
        Detail.Children.Clear();
        if (info is null)
            return;
        var row = info.Row;
        Detail.Children.Add(Ui.Text(info.Name, "SubtitleTextBlockStyle"));
        var meta = info.Origin;
        if (row.Version is { Length: > 0 } version)
            meta += $" · version {version}";
        Detail.Children.Add(Ui.Haze(meta));

        var facts = new StackPanel { Spacing = 10 };
        var standing = Standing(info);
        var state = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        state.Children.Add(Ui.Text("Standing"));
        state.Children.Add(standing.Children.Count > 0 ? standing : Ui.Haze("Working"));
        facts.Children.Add(state);
        if (row.LatencyMs is { } ms)
            facts.Children.Add(Ui.Haze($"Answers in {ms} ms"));
        if (row.State == "down" && row.LastError is { } error)
            facts.Children.Add(Ui.Haze(char.ToUpperInvariant(error[0]) + error[1..]));
        if (row.HeldBack)
            facts.Children.Add(Ui.Haze("Held back until it answers again."));
        if (!info.IsTorrent)
        {
            var hosts = info.Hosts.Count > 0 ? info.Hosts : row.Hosts ?? [];
            facts.Children.Add(Ui.Haze(hosts.Count > 0 ? "Contacts " + string.Join(", ", hosts) : "Contacts no hosts"));
        }
        else
            facts.Children.Add(Ui.Haze("Searches the torrent indexes you have turned on under Settings."));
        Detail.Children.Add(Ui.Card(facts));

        if (info.Update is { } next && info.Store is { } store && info.Listing is { } listing)
        {
            var update = new StackPanel { Spacing = 8 };
            update.Children.Add(Ui.Text($"Version {next} is ready", "BodyStrongTextBlockStyle"));
            if (row.Update?.NewHosts is { Length: > 0 } added)
                update.Children.Add(Ui.Haze("It would also contact " + string.Join(", ", added) + "."));
            update.Children.Add(Ui.Label("Update", (_, _) => ctx.Send(new Command.InstallPlugin(store.Id, listing.Id))));
            Detail.Children.Add(Ui.Card(update));
        }

        if (info.IsPlugin)
        {
            var off = new ToggleSwitch { IsOn = !row.Disabled, OnContent = "On", OffContent = "Off" };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(off, $"{info.Name} on");
            off.Toggled += (_, _) =>
            {
                if (!quiet)
                    ctx.Send(new Command.DisablePlugin(row.Id, !off.IsOn));
            };
            Detail.Children.Add(Ui.Card(off, 10));
            var remove = Ui.Label("Remove", (_, _) => ConfirmRemove(info));
            remove.HorizontalAlignment = HorizontalAlignment.Left;
            Detail.Children.Add(remove);
        }
    }

    async void ConfirmRemove(ProviderInfo info)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = $"Remove {info.Name}?",
            Content = "It stops being tried for episodes. You can install it again from its store.",
            PrimaryButtonText = "Remove",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
            ctx.Send(new Command.RemovePlugin(info.Row.Id));
    }

    void OnSelected(object sender, SelectionChangedEventArgs e)
    {
        if (quiet || Order.SelectedItem is not ListViewItem { Tag: string id })
            return;
        selected = id;
        ShowDetail(ctx.Known.OrderedProviders.FirstOrDefault(r => r.Id == id) is { } row ? Info(row) : null);
    }

    void OnDragCompleted(ListViewBase sender, DragItemsCompletedEventArgs args)
    {
        var order = Order.Items.OfType<ListViewItem>().Select(i => (string)i.Tag).ToArray();
        if (!order.SequenceEqual(ctx.Known.OrderedProviders.Select(r => r.Id)))
            ctx.Send(new Command.SetSetting(SettingKey.SourceOrder, new SettingValue.Strings(order)));
    }

    void OnOrderKey(object sender, KeyRoutedEventArgs e)
    {
        var alt = Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(global::Windows.System.VirtualKey.Menu)
            .HasFlag(global::Windows.UI.Core.CoreVirtualKeyStates.Down);
        if (!alt || selected is null || e.Key is not (global::Windows.System.VirtualKey.Up or global::Windows.System.VirtualKey.Down))
            return;
        var step = e.Key == global::Windows.System.VirtualKey.Up ? -1 : 1;
        if (SourceOrder.Moved(ctx.Known.Settings?.Sources.Order ?? [], ctx.Known.Providers, selected, step) is { } moved)
            ctx.Send(new Command.SetSetting(SettingKey.SourceOrder, new SettingValue.Strings([.. moved])));
        e.Handled = true;
    }

    void OnCheck(object sender, RoutedEventArgs e) => ctx.Send(new Command.CheckProviders());
}
