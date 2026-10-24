using Microsoft.UI.Composition;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace Anistream.Windows;

/// <summary>
/// Mica tinted towards the ground, as the brief sets it: tint #161A2E at 0.8, falling back to the
/// solid ground when the window is inactive, transparency is off or the battery saver is on.
/// It shows only where nothing is drawn over it: the title bar and the navigation pane.
/// </summary>
sealed partial class TintedMica : SystemBackdrop
{
    static readonly Color Ground = Color.FromArgb(0xFF, 0x16, 0x1A, 0x2E);
    MicaController? controller;

    protected override void OnTargetConnected(ICompositionSupportsSystemBackdrop target, XamlRoot root)
    {
        base.OnTargetConnected(target, root);
        if (!MicaController.IsSupported())
            return;
        controller = new MicaController
        {
            Kind = MicaKind.Base,
            TintColor = Ground,
            TintOpacity = 0.8f,
            LuminosityOpacity = 1f,
            FallbackColor = Ground,
        };
        controller.SetSystemBackdropConfiguration(GetDefaultSystemBackdropConfiguration(target, root));
        controller.AddSystemBackdropTarget(target);
    }

    protected override void OnTargetDisconnected(ICompositionSupportsSystemBackdrop target)
    {
        base.OnTargetDisconnected(target);
        controller?.RemoveSystemBackdropTarget(target);
        controller?.Dispose();
        controller = null;
    }
}
