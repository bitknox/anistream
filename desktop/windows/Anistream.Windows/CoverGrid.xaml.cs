using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows;

public sealed partial class CoverGrid : UserControl
{
    public CoverGrid()
    {
        InitializeComponent();
    }

    /// <summary>The covers, replaced whole.</summary>
    internal IReadOnlyList<CoverItem> Covers
    {
        set => Items.ItemsSource = value;
    }

    void OnInvoked(ItemsView sender, ItemsViewItemInvokedEventArgs e)
    {
        if (e.InvokedItem is CoverItem item)
            Ctx.Current.OpenTitle(item.Id);
    }

    void OnOpen(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is CoverItem item)
            Ctx.Current.OpenTitle(item.Id);
    }

    void OnPlan(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is CoverItem item)
            Ctx.Current.Send(new Command.SetStatus(item.Id, LibrarySegment.Planning));
    }
}
