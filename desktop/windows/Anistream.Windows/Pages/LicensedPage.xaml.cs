using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Anistream.Windows.Pages;

/// <summary>
/// Licensed services: Region, then Your services as check rows built from AniList's list (the
/// first eight, then Show all), then the switch for the line that names services the viewer does
/// not use. The engine keeps the choices; nothing here plays a service.
/// </summary>
public sealed partial class LicensedPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    bool all, quiet;

    public LicensedPage() => InitializeComponent();

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ctx.Hear(this, batch =>
        {
            if (batch.Any(x => x is Event.Licensed))
                Draw();
        });
        ctx.Send(new Command.Licensed(new LicensedCommand.Load(System.Globalization.RegionInfo.CurrentRegion.TwoLetterISORegionName)));
        Draw();
    }

    void Draw()
    {
        quiet = true;
        Stack.Children.Clear();
        var title = Ui.Text(T.LicensedTitle, "PageTitleStyle");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetHeadingLevel(title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level1);
        Stack.Children.Add(title);
        Stack.Children.Add(Ui.Text(T.LicensedBody, "BodyTextBlockStyle"));
        if (ctx.Known.Licensed is not { } view)
        {
            quiet = false;
            return;
        }

        var region = new ComboBox { Header = T.LicensedRegion, MinWidth = 260, Margin = new Thickness(0, 12, 0, 0) };
        var codes = view.Regions.Length > 0 ? view.Regions : ["DK"];
        foreach (var code in codes)
            region.Items.Add(new ComboBoxItem { Content = Lawful.RegionName(code), Tag = code });
        region.SelectedItem = region.Items.OfType<ComboBoxItem>().FirstOrDefault(i => (string)i.Tag == view.Region);
        region.SelectionChanged += (_, _) =>
        {
            if (!quiet && region.SelectedItem is ComboBoxItem { Tag: string code })
                ctx.Send(new Command.Licensed(new LicensedCommand.SetRegion(code)));
        };
        Stack.Children.Add(region);
        Stack.Children.Add(Ui.Haze(T.LicensedRegionLine));

        Stack.Children.Add(Ui.Heading(T.LicensedYours));
        var services = view.Available.Concat(view.Settings.Services).Distinct(StringComparer.OrdinalIgnoreCase).OrderBy(s => s, StringComparer.CurrentCultureIgnoreCase).ToArray();
        var shown = all ? services : services.Take(8).ToArray();
        foreach (var service in shown)
        {
            var box = new CheckBox { Content = service, IsChecked = view.Settings.Services.Contains(service) };
            box.Click += (_, _) => SaveServices();
            Stack.Children.Add(box);
        }
        if (!all && services.Length > shown.Length)
        {
            var more = new HyperlinkButton { Content = T.LicensedShowAll(services.Length), Padding = new Thickness(0, 6, 0, 6) };
            more.Click += (_, _) => { all = true; Draw(); };
            Stack.Children.Add(more);
        }

        var others = new ToggleSwitch { IsOn = view.Settings.ShowOthers, OnContent = "", OffContent = "", Header = T.LicensedShowOthers, Margin = new Thickness(0, 16, 0, 0) };
        others.Toggled += (_, _) =>
        {
            if (!quiet)
                ctx.Send(new Command.Licensed(new LicensedCommand.SetShowOthers(others.IsOn)));
        };
        Stack.Children.Add(others);
        Stack.Children.Add(Ui.Haze(T.LicensedShowOthersLine));
        quiet = false;
    }

    void SaveServices()
    {
        var ticked = Stack.Children.OfType<CheckBox>().Where(b => b.IsChecked == true).Select(b => (string)b.Content).ToArray();
        // Services chosen but not on the shown list stay chosen.
        var hidden = ctx.Known.Licensed?.Settings.Services.Where(s => !Stack.Children.OfType<CheckBox>().Any(b => (string)b.Content == s)) ?? [];
        ctx.Send(new Command.Licensed(new LicensedCommand.SetServices([.. ticked, .. hidden])));
    }
}
