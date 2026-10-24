using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;

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
        if (e.InvokedItem is not CoverItem item)
            return;
        // The cover flies to the title page's (a connected animation), when animations are on.
        if (Motion.Enabled && CoverOf(item) is { } cover)
            ConnectedAnimationService.GetForCurrentView().PrepareToAnimate(Pages.TitlePage.CoverAnimation, cover);
        Ctx.Current.OpenTitle(item.Id);
    }

    /// <summary>The cover art of the shown container for <paramref name="item"/>, if it is realised.</summary>
    UIElement? CoverOf(CoverItem item)
    {
        static IEnumerable<DependencyObject> Below(DependencyObject root)
        {
            var count = VisualTreeHelper.GetChildrenCount(root);
            for (var i = 0; i < count; i++)
            {
                var child = VisualTreeHelper.GetChild(root, i);
                yield return child;
                foreach (var deeper in Below(child))
                    yield return deeper;
            }
        }
        var container = Below(Items).OfType<ItemContainer>().FirstOrDefault(c => c.DataContext == item);
        return container is null ? null : Below(container).OfType<FrameworkElement>().FirstOrDefault(f => f.Tag as string == "cover");
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
