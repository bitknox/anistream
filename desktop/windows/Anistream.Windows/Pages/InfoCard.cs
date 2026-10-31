using Anistream.App;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Anistream.Windows.Pages;

/// <summary>
/// A surface with nothing to lose (Light dismiss, native-ux.md): a card in the middle of the
/// window over the dimmed window, which a click outside it, Esc or Close closes. A ContentDialog
/// has no light dismiss, so informational surfaces use this light-dismiss Popup instead. It looks
/// like the app's dialogs: their fill, border and corners, the title over the content, and Close
/// at the foot.
/// </summary>
static class InfoCard
{
    /// <summary>The cards that are up, which <see cref="HideOpen"/> closes.</summary>
    static readonly List<Popup> open = [];

    /// <summary>Whether a card is up.</summary>
    public static bool IsOpen => open.Count > 0;

    /// <summary>Shows the card, and completes once it closes. <paramref name="content"/> scrolls itself.</summary>
    public static Task Show(XamlRoot root, FrameworkElement title, FrameworkElement content, double maxWidth, double maxHeight)
    {
        var done = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var close = new Button
        {
            Content = T.ActionClose,
            Width = 120,
            Height = 32,
            HorizontalAlignment = HorizontalAlignment.Right,
        };
        var layout = new Grid { RowSpacing = 16 };
        layout.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        layout.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        layout.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        // The content scrolls itself, as a ContentDialog's does.
        Grid.SetRow(title, 0);
        Grid.SetRow(content, 1);
        Grid.SetRow(close, 2);
        layout.Children.Add(title);
        layout.Children.Add(content);
        layout.Children.Add(close);
        var card = new Border
        {
            Background = Ui.Res<Brush>("ContentDialogBackground"),
            BorderBrush = Ui.Res<Brush>("ContentDialogBorderBrush"),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(24),
            Child = layout,
        };
        AutomationProperties.SetIsDialog(card, true);
        var popup = new Popup
        {
            XamlRoot = root,
            Child = card,
            IsLightDismissEnabled = true,
            LightDismissOverlayMode = LightDismissOverlayMode.On,
            ShouldConstrainToRootBounds = true,
        };

        // In the middle of the window, inside it by 24 on every side.
        void Place()
        {
            var size = root.Size;
            card.MaxWidth = Math.Max(0, Math.Min(maxWidth, size.Width - 48));
            card.MaxHeight = Math.Max(0, Math.Min(maxHeight, size.Height - 48));
            card.Measure(new global::Windows.Foundation.Size(card.MaxWidth, card.MaxHeight));
            popup.HorizontalOffset = Math.Max(0, (size.Width - card.DesiredSize.Width) / 2);
            popup.VerticalOffset = Math.Max(0, (size.Height - card.DesiredSize.Height) / 2);
        }
        void Resized(XamlRoot sender, XamlRootChangedEventArgs e) => Place();

        close.Click += (_, _) => popup.IsOpen = false;
        card.KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.Escape)
            {
                popup.IsOpen = false;
                e.Handled = true;
            }
        };
        card.SizeChanged += (_, _) => Place();
        root.Changed += Resized;
        popup.Closed += (_, _) =>
        {
            root.Changed -= Resized;
            open.Remove(popup);
            done.TrySetResult();
        };
        Place();
        open.Add(popup);
        popup.IsOpen = true;
        close.Focus(FocusState.Programmatic);
        return done.Task;
    }

    /// <summary>Closes the cards that are up: whether one was.</summary>
    public static bool HideOpen()
    {
        var any = open.Count > 0;
        foreach (var popup in open.ToArray())
            popup.IsOpen = false;
        return any;
    }
}
