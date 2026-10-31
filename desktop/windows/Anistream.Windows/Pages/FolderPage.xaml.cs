using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Anistream.Windows.Pages;

/// <summary>
/// One folder of Your media: its path and state, Rescan, Show in Explorer and Remove folder, then
/// the lists Needs a look, Not matched and Matched. Choosing a match opens the fix dialog.
/// </summary>
public sealed partial class FolderPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    string path = "";
    bool matchedOpen;

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        path = e.Parameter as string ?? "";
        ctx.Hear(this, batch =>
        {
            if (batch.Any(x => x is Event.Local))
                Draw();
        });
        ctx.Send(new Command.Local(new LocalCommand.Load()));
        Draw();
    }

    public FolderPage() => InitializeComponent();

    void Draw()
    {
        Stack.Children.Clear();
        var view = ctx.Known.Local;
        var folder = view?.Folders.FirstOrDefault(f => f.Path == path);
        var title = Ui.Text(Path.GetFileName(path.TrimEnd('\\', '/')) is { Length: > 0 } name ? name : path, "PageTitleStyle");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetHeadingLevel(title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level1);
        Stack.Children.Add(title);
        Stack.Children.Add(Ui.Haze(path, wrap: true));
        if (folder is null || view is null)
        {
            // Removed while the page was up.
            if (view is not null)
                Stack.Children.Add(Ui.Text(T.LibraryRowMissing, "BodyTextBlockStyle"));
            return;
        }
        var state = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        if (!folder.Available)
            state.Children.Add(Ui.Dot(Colours.Haze, ring: true));
        state.Children.Add(Ui.Text(Lawful.FolderLine(folder, view), "BodyTextBlockStyle"));
        Stack.Children.Add(state);

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 4, 0, 0) };
        actions.Children.Add(Ui.Label(T.LibraryRescan, (_, _) => ctx.Send(new Command.Local(new LocalCommand.Scan()))));
        actions.Children.Add(Ui.Label(T.LibraryShowExplorer, (_, _) => ShowInExplorer(), accent: false));
        actions.Children.Add(Ui.Label(T.LibraryRemoveFolder, (_, _) => Remove()));
        Stack.Children.Add(actions);
        Stack.Children.Add(Ui.Haze(T.LibraryRemoveDetail));

        Section(T.LibraryNeedsLook, view.Groups.Where(g => g.State == GroupState.NeedsLook), choose: true, keep: false);
        Section(T.LibraryNotMatched, view.Groups.Where(g => g.State == GroupState.NotMatched), choose: true, keep: true);
        var matched = view.Groups.Where(g => g.State is GroupState.Matched or GroupState.Kept).ToArray();
        if (matched.Length > 0)
        {
            var open = new HyperlinkButton { Content = T.LibraryMatched(matched.Length), Padding = new Thickness(0, 8, 0, 8), Margin = new Thickness(0, 12, 0, 0) };
            open.Click += (_, _) => { matchedOpen = !matchedOpen; Draw(); };
            Stack.Children.Add(open);
            if (matchedOpen)
                foreach (var group in matched)
                    Stack.Children.Add(Row(group, choose: false, keep: false, fix: true));
        }
    }

    void Section(string heading, IEnumerable<LocalGroup> groups, bool choose, bool keep)
    {
        var list = groups.ToArray();
        if (list.Length == 0)
            return;
        Stack.Children.Add(Ui.Heading(heading));
        foreach (var group in list)
            Stack.Children.Add(Row(group, choose, keep, fix: false));
    }

    UIElement Row(LocalGroup group, bool choose, bool keep, bool fix)
    {
        var grid = new Grid { ColumnSpacing = 12, MinHeight = 52, Padding = new Thickness(0, 6, 0, 6) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(Ui.Text(group.MatchedTitle is { Length: > 0 } t && group.State is GroupState.Matched ? t : group.Name, "BodyStrongTextBlockStyle", wrap: false));
        var line = T.LibraryFiles(group.Files);
        if (group.State == GroupState.NotMatched)
            line = $"{Lawful.Reason(group.Reason)} · {line}";
        text.Children.Add(Ui.Text(line, "HazeCaptionStyle", wrap: false));
        grid.Children.Add(text);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        if (choose || fix)
            buttons.Children.Add(Ui.Label(fix ? T.MatchFix : T.MatchChoose, (_, _) => Fix(group)));
        if (keep)
        {
            var more = new Button { Content = new FontIcon { Glyph = "", FontSize = 14 }, Width = 36, Height = 32, Padding = new Thickness(0) };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(more, T.ActionMore);
            var menu = new MenuFlyout();
            var item = new MenuFlyoutItem { Text = T.LibraryKeepUnmatched };
            item.Click += (_, _) => ctx.Send(new Command.Local(new LocalCommand.Keep(group.Id)));
            menu.Items.Add(item);
            more.Flyout = menu;
            buttons.Children.Add(more);
        }
        Grid.SetColumn(buttons, 1);
        grid.Children.Add(buttons);
        return grid;
    }

    async void Fix(LocalGroup group)
    {
        var dialog = new FixMatchDialog(ctx, XamlRoot, group);
        await dialog.ShowAsync();
    }

    void ShowInExplorer()
    {
        try
        {
            System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo("explorer.exe", $"\"{path}\"") { UseShellExecute = true });
        }
        catch (Exception)
        {
            // No Explorer to show it in: the path is on the page.
        }
    }

    void Remove()
    {
        ctx.Send(new Command.Local(new LocalCommand.RemoveFolder(path)));
        // Undo adds the folder back, which scans it again; the files were never touched.
        var back = path;
        ctx.Window.Say(T.SourcesRemoved(Path.GetFileName(path.TrimEnd('\\', '/')) is { Length: > 0 } n ? n : path) + " " + T.SourcesRemovedFilesStay,
            T.ActionUndo, () => ctx.Send(new Command.Local(new LocalCommand.AddFolder(back))));
        ctx.Go("providers");
    }
}
