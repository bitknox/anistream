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
/// Your sources: one page with a section per kind, in the order the board draws them. Your media,
/// Media servers and Licensed services rows open their own pages. A store's plugins and what is
/// built in are rows in the order failover tries them, moved by dragging or by Alt with the up and
/// down arrows; the selected one's version, speed, what it may contact and what can be done to it
/// (update, switch off, remove) is beside the list.
/// </summary>
public sealed partial class ProvidersPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    string? selected;
    bool quiet;
    readonly List<ListView> lists = [];

    public ProvidersPage()
    {
        InitializeComponent();
        ctx.Hear(this, Heard);
        Sections.KeyDown += OnOrderKey;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        selected = ctx.Options.Select;
        ctx.Send(new Command.CheckProviders());
        ctx.Send(new Command.Local(new LocalCommand.Load()));
        ctx.Send(new Command.Server(new ServerCommand.Load()));
        ctx.Send(new Command.Licensed(new LicensedCommand.Load(System.Globalization.RegionInfo.CurrentRegion.TwoLetterISORegionName)));
        if (!ctx.Known.StoresLoaded)
            ctx.Send(new Command.LoadStores());
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.Providers or Event.ProviderNote or Event.SourcesChanged or Event.Stores or Event.Settings or Event.Local or Event.Server or Event.Licensed))
            Show();
    }

    ProviderInfo Info(ProviderRow row) => ProviderInfo.Of(row, ctx.Known.Stores);

    void Show()
    {
        quiet = true;
        var rows = ctx.Known.OrderedProviders;
        Rebuilding.Visibility = ctx.Known.Rebuilding ? Visibility.Visible : Visibility.Collapsed;
        Note.Visibility = Visibility.Collapsed;
        var folders = ctx.Known.Local?.Folders ?? [];
        var servers = ctx.Known.Servers;
        // No sources at all: the spec's empty state, not the engine's note.
        var none = ctx.Known.ProvidersLoaded && AnistreamFfiMethods.HasNoSources(rows) && folders.Length == 0 && servers.Length == 0;
        NoSources.Visibility = none ? Visibility.Visible : Visibility.Collapsed;
        Columns.Visibility = none ? Visibility.Collapsed : Visibility.Visible;
        if (none && NoSources.Child is null)
            NoSources.Child = Empty();
        var current = rows.FirstOrDefault(r => r.Id == selected && r.Kind is "plugin" or "torrent" or "remote" or "demo");
        selected = current?.Id;
        Sections.Children.Clear();
        lists.Clear();

        if (ctx.Known.Local is { Folders.Length: > 0 } local)
            FirstParty(T.SourcesSectionMedia, local.Folders.Select(f => (Glyph: "\uE8B7", Title: f.Path, Line: Lawful.FolderLine(f, local), Quiet: !f.Available, Page: typeof(FolderPage), Param: (object)f.Path)));
        if (servers.Length > 0)
            FirstParty(T.SourcesSectionServers, servers.Select(s => (Glyph: "\uE839", Title: s.Name, Line: s.SignedOut ? T.ServersRowSignedOut : Lawful.ServerMatching(s) ?? Lawful.ServerLine(s), Quiet: false, Page: typeof(ServerPage), Param: (object)s.Id)));
        if (ctx.Known.Licensed is { } licensed)
            FirstParty(T.SourcesSectionLicensed, [(Glyph: "\uE774", Title: T.LicensedTitle, Line: Lawful.LicensedLine(licensed), Quiet: licensed.Settings.Services.Length == 0, Page: typeof(LicensedPage), Param: (object)"")]);

        var shown = rows.Where(r => r.Kind is "plugin" or "torrent" or "remote" or "demo").ToArray();
        foreach (var group in AnistreamFfiMethods.SourceGroups(shown))
        {
            if (group.Section is not (SourceSection.Plugins or SourceSection.BuiltIn))
                continue;
            var ids = group.Ids;
            var heading = group.Section == SourceSection.BuiltIn ? T.SourcesSectionBuiltin
                : group.Store is { } store ? T.SourcesSectionStore(ctx.Known.Stores.FirstOrDefault(x => x.Id == store)?.Name ?? shown.Select(r => r.Origin).OfType<ProviderOrigin.Store>().FirstOrDefault(o => o.Id == store)?.Name ?? store)
                : T.SourcesOriginByHand;
            var list = NewList(group.Section == SourceSection.Plugins);
            foreach (var row in ids.Select(id => shown.First(r => r.Id == id)))
            {
                var item = new ListViewItem { Content = Line(Info(row), row.Id == current?.Id), Tag = row.Id };
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(item, Info(row).Name);
                list.Items.Add(item);
                if (row.Id == current?.Id)
                    list.SelectedItem = item;
            }
            Sections.Children.Add(Ui.Heading(heading));
            Sections.Children.Add(list);
        }
        ShowDetail(current is null ? null : Info(current));
        quiet = false;
    }

    ListView NewList(bool reorder)
    {
        var list = new ListView { SelectionMode = ListViewSelectionMode.Single, CanReorderItems = reorder, AllowDrop = reorder, Margin = new Thickness(-12, 0, 0, 0) };
        list.SelectionChanged += OnSelected;
        if (reorder)
            list.DragItemsCompleted += OnDragCompleted;
        lists.Add(list);
        return list;
    }

    /// <summary>A section of rows that open a page: no drag handle, a chevron instead.</summary>
    void FirstParty(string heading, IEnumerable<(string Glyph, string Title, string Line, bool Quiet, Type Page, object Param)> rows)
    {
        var list = new ListView { SelectionMode = ListViewSelectionMode.None, IsItemClickEnabled = true, Margin = new Thickness(-12, 0, 0, 0) };
        list.ItemClick += (_, e) =>
        {
            if (e.ClickedItem is ListViewItem { Tag: ValueTuple<Type, object> go })
                ctx.Window.Push(go.Item1, go.Item2 is "" ? null : go.Item2);
        };
        foreach (var row in rows)
        {
            var grid = new Grid { Height = 52, ColumnSpacing = 12 };
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            grid.Children.Add(new FontIcon { Glyph = row.Glyph, FontSize = 14, VerticalAlignment = VerticalAlignment.Center, Foreground = Ui.Res<Microsoft.UI.Xaml.Media.Brush>("TextFillColorSecondaryBrush") });
            var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
            text.Children.Add(Ui.Text(row.Title, "BodyStrongTextBlockStyle", wrap: false));
            text.Children.Add(Ui.Text(row.Line, "HazeCaptionStyle", wrap: false));
            Grid.SetColumn(text, 1);
            grid.Children.Add(text);
            var chevron = new FontIcon { Glyph = "\uE76C", FontSize = 12, VerticalAlignment = VerticalAlignment.Center, Foreground = Ui.Res<Microsoft.UI.Xaml.Media.Brush>("TextFillColorSecondaryBrush") };
            Grid.SetColumn(chevron, 2);
            grid.Children.Add(chevron);
            var item = new ListViewItem { Content = grid, Tag = (row.Page, row.Param) };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(item, $"{row.Title}, {row.Line}");
            list.Items.Add(item);
        }
        Sections.Children.Add(Ui.Heading(heading));
        Sections.Children.Add(list);
    }

    /// <summary>No sources yet: Add a folder (amber) and Add a media server, then a line to join a store.</summary>
    UIElement Empty()
    {
        var empty = new StackPanel { Spacing = 12, MaxWidth = 520, HorizontalAlignment = HorizontalAlignment.Left, Margin = new Thickness(0, 40, 0, 0) };
        empty.Children.Add(Ui.Text(T.SourcesEmptyTitle, "SubtitleTextBlockStyle"));
        empty.Children.Add(Ui.Haze(T.SourcesEmptyBody));
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 8, 0, 0) };
        buttons.Children.Add(Ui.Label(T.SourcesAddFolderButton, (_, _) => ctx.Window.AddFolder(), accent: true));
        buttons.Children.Add(Ui.Label(T.SourcesAddServerButton, (_, _) => ctx.Window.ShowAddServer()));
        empty.Children.Add(buttons);
        var more = new StackPanel { Orientation = Orientation.Horizontal };
        var join = new HyperlinkButton { Content = T.SourcesEmptyJoinLink, Padding = new Thickness(0) };
        join.Click += (_, _) => ctx.Window.ShowJoinStore();
        // "Or { join }.": the link stands where the sentence puts it.
        var parts = T.SourcesEmptyMore("\u0001").Split('\u0001');
        more.Children.Add(Ui.Haze(parts[0], wrap: false));
        more.Children.Add(join);
        if (parts.Length > 1)
            more.Children.Add(Ui.Haze(parts[1], wrap: false));
        empty.Children.Add(more);
        var legal = Ui.Text(T.LegalHostsNoContent, "HazeCaptionStyle");
        legal.Margin = new Thickness(0, 16, 0, 0);
        empty.Children.Add(legal);
        return empty;
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
            line.Children.Add(Ui.Text(T.SourcesStatusUpdate, "HazeCaptionStyle", wrap: false));
        }
        return line;
    }

    void ShowDetail(ProviderInfo? info)
    {
        Detail.Children.Clear();
        if (info is null)
            return;
        var row = info.Row;
        Detail.Children.Add(Ui.Text(info.Name, "SubtitleTextBlockStyle"));
        Detail.Children.Add(Ui.Haze(row.Version is { Length: > 0 } version ? T.SourcesOriginVersion(info.Origin, version) : info.Origin));

        var facts = new StackPanel { Spacing = 10 };
        var standing = Standing(info);
        var state = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        state.Children.Add(Ui.Text(T.SourcesStanding));
        state.Children.Add(standing.Children.Count > 0 ? standing : Ui.Haze(T.SourcesStatusWorking));
        facts.Children.Add(state);
        // The engine's error is English from deep in the request: the line says what it means.
        if (row.State == "down" && row.LastError is not null)
            facts.Children.Add(Ui.Haze(T.SourcesDetailSite));
        if (row.HeldBack)
            facts.Children.Add(Ui.Haze(T.SourcesDetailHeldBack));
        if (!info.IsTorrent)
        {
            var hosts = info.Hosts.Count > 0 ? info.Hosts : row.Hosts ?? [];
            facts.Children.Add(Ui.Haze(hosts.Count > 0 ? T.SourcesContactsOne(string.Join(", ", hosts)) : T.SourcesContactsNone));
        }
        else
            facts.Children.Add(Ui.Haze(T.SourcesTorrentIndexes));
        Detail.Children.Add(Ui.Card(facts));

        if (info.Update is { } next && info.Store is { } store && info.Listing is { } listing)
        {
            var update = new StackPanel { Spacing = 8 };
            update.Children.Add(Ui.Text(T.SourcesVersionReady(next), "BodyStrongTextBlockStyle"));
            if (row.Update?.NewHosts is { Length: > 0 } added)
                update.Children.Add(Ui.Haze(T.StoresUpdateContacts(string.Join(", ", added))));
            var install = Ui.Label(T.ActionUpdate, (_, _) => ConfirmUpdate(info, store.Id, listing.Id, row.Update?.NewHosts ?? []), accent: true);
            install.HorizontalAlignment = HorizontalAlignment.Left;
            update.Children.Add(install);
            Detail.Children.Add(Ui.Card(update));
        }

        if (info.IsPlugin)
        {
            var off = new ToggleSwitch { IsOn = !row.Disabled, OnContent = "", OffContent = "", Header = T.SourcesUseThis };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(off, T.SourcesUseName(info.Name));
            off.Toggled += (_, _) =>
            {
                if (!quiet)
                    ctx.Send(new Command.DisablePlugin(row.Id, !off.IsOn));
            };
            Detail.Children.Add(Ui.Card(off, 10));
            var remove = Ui.Label(T.ActionRemove, (_, _) => ConfirmRemove(info));
            remove.HorizontalAlignment = HorizontalAlignment.Left;
            Detail.Children.Add(remove);
        }
    }

    /// <summary>An update that would contact new hosts says which, and asks; otherwise it installs.</summary>
    async void ConfirmUpdate(ProviderInfo info, string store, string plugin, string[] added)
    {
        if (added.Length > 0)
        {
            var dialog = new ContentDialog
            {
                XamlRoot = XamlRoot,
                Title = T.SourcesUpdateTitle(info.Name),
                Content = Ui.Text(T.StoresUpdateContacts(string.Join(", ", added)), "BodyTextBlockStyle"),
                PrimaryButtonText = T.ActionUpdate,
                CloseButtonText = T.ActionCancel,
                DefaultButton = ContentDialogButton.Close,
            };
            if (await Dialogs.Show(dialog) != ContentDialogResult.Primary)
                return;
        }
        ctx.Send(new Command.InstallPlugin(store, plugin));
    }

    /// <summary>Below about 760 the detail goes under the list, as one column.</summary>
    void OnColumnsSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var narrow = e.NewSize.Width < 760;
        ListColumn.Width = narrow ? new GridLength(1, GridUnitType.Star) : new GridLength(420);
        DetailColumn.Width = narrow ? new GridLength(0) : new GridLength(1, GridUnitType.Star);
        ListRow.Height = narrow ? GridLength.Auto : new GridLength(1, GridUnitType.Star);
        DetailRow.Height = narrow ? new GridLength(1, GridUnitType.Star) : new GridLength(0);
        Grid.SetColumn(DetailScroll, narrow ? 0 : 1);
        Grid.SetRow(DetailScroll, narrow ? 1 : 0);
        ListScroll.MaxHeight = narrow ? Math.Max(200, e.NewSize.Height * 0.45) : double.PositiveInfinity;
    }

    async void ConfirmRemove(ProviderInfo info)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = T.StoresRemovePluginTitle(info.Name),
            Content = T.SourcesRemoveDetail,
            PrimaryButtonText = T.ActionRemove,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Close,
        };
        if (await Dialogs.Show(dialog) == ContentDialogResult.Primary)
            ctx.Send(new Command.RemovePlugin(info.Row.Id));
    }

    void OnSelected(object sender, SelectionChangedEventArgs e)
    {
        if (quiet || sender is not ListView { SelectedItem: ListViewItem { Tag: string id } } list)
            return;
        selected = id;
        quiet = true;
        foreach (var other in lists.Where(l => l != list))
            other.SelectedItem = null;
        quiet = false;
        ShowDetail(ctx.Known.OrderedProviders.FirstOrDefault(r => r.Id == id) is { } row ? Info(row) : null);
    }

    void OnDragCompleted(ListViewBase sender, DragItemsCompletedEventArgs args)
    {
        var rows = ctx.Known.OrderedProviders;
        var everything = rows.Select(r => r.Id).ToList();
        var after = sender.Items.OfType<ListViewItem>().Select(i => (string)i.Tag).ToList();
        var before = everything.Where(after.Contains).ToList();
        if (before.SequenceEqual(after) || args.Items.FirstOrDefault() is not ListViewItem { Tag: string id })
            return;
        var index = SourceOrder.DropIndex(everything, before, after, id);
        if (SourceOrder.Placed(ctx.Known.Settings?.Sources.Order ?? [], rows, id, index) is { } placed)
            ctx.Send(new Command.SetSetting(SettingKey.SourceOrder, new SettingValue.Strings([.. placed])));
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

    void OnAddFolder(object sender, RoutedEventArgs e) => ctx.Window.AddFolder();

    void OnAddServer(object sender, RoutedEventArgs e) => ctx.Window.ShowAddServer();

    void OnAddStore(object sender, RoutedEventArgs e) => ctx.Window.ShowJoinStore();
}
