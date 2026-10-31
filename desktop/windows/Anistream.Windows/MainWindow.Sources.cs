using Anistream.App;
using Anistream.Engine.Model;
using Anistream.Windows.Pages;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;

namespace Anistream.Windows;

/// <summary>The window's part in first-party sources: pushed pages, the folder picker, the add-server dialog and the return from a licensed service.</summary>
public sealed partial class MainWindow
{
    /// <summary>A page pushed over the current one, with the back button back to it.</summary>
    internal void Push(Type page, object? parameter = null) =>
        ContentFrame.Navigate(page, parameter, new SuppressNavigationTransitionInfo());

    /// <summary>The system's folder picker, or null when it is cancelled. UNC paths are accepted.</summary>
    internal async Task<string?> PickFolder()
    {
        var picker = new global::Windows.Storage.Pickers.FolderPicker { SuggestedStartLocation = global::Windows.Storage.Pickers.PickerLocationId.VideosLibrary };
        picker.FileTypeFilter.Add("*");
        WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(this));
        var folder = await picker.PickSingleFolderAsync();
        return folder?.Path;
    }

    /// <summary>The Add menu's Folder…: pick, then hand the path to the engine, which scans and matches it.</summary>
    internal async void AddFolder()
    {
        if (await PickFolder() is { Length: > 0 } path)
        {
            ctx.Send(new Command.Local(new LocalCommand.AddFolder(path)));
            Go("providers");
        }
    }

    AddServerDialog? addingServer;

    /// <summary>The add-server dialog, at the sign-in step for a server that signed the viewer out when given its address.</summary>
    internal async void ShowAddServer(string? address = null)
    {
        var dialog = addingServer = new AddServerDialog(ctx, Root.XamlRoot, address);
        try
        {
            await dialog.ShowAsync();
        }
        catch (System.Runtime.InteropServices.COMException)
        {
            return;
        }
        finally
        {
            if (addingServer == dialog)
                addingServer = null;
        }
    }

    /// <summary>Opens a service's page in the default browser, or the app that has claimed its links.</summary>
    internal static async void OpenOutside(string url)
    {
        if (Uri.TryCreate(url, UriKind.Absolute, out var uri) && uri.Scheme is "https" or "http")
            await global::Windows.System.Launcher.LaunchUriAsync(uri);
    }

    /// <summary>Back from a licensed service: "Watched episode 4 on Hikari Stream?" holds Mark watched for 8 s.</summary>
    void ShowReturn(LicensedReturn back) =>
        Say(back.Prompt.Text(), T.LicensedMarkWatched, () =>
        {
            ctx.Send(new Command.SetWatched(back.Id, [back.Episode.ToString()], true));
            Say(back.Done.Text());
        });
}
