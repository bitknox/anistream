using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows.Pages;

/// <summary>
/// Every ContentDialog opens through here. WinUI allows one open dialog at a time and throws
/// from <c>ShowAsync</c> for a second, which inside an <c>async void</c> handler ends the app.
/// A dialog asked for while another is up waits for it to close; one that must come first (the
/// question closing the window asks) closes the other.
/// </summary>
static class Dialogs
{
    static ContentDialog? open;
    static TaskCompletionSource? closed;

    /// <summary>Whether one of the app's dialogs is up.</summary>
    public static bool Open => open is not null;

    /// <summary>
    /// Shows <paramref name="dialog"/> once no other is up, and its answer. <paramref name="shown"/>
    /// runs as it opens. With <paramref name="first"/> an open dialog is closed (as Cancel) to make
    /// way. A dialog WinUI refuses all the same counts as cancelled.
    /// </summary>
    public static async Task<ContentDialogResult> Show(ContentDialog dialog, Action? shown = null, bool first = false)
    {
        while (open is { } other && closed is { } wait)
        {
            if (other == dialog)
                return ContentDialogResult.None;
            if (first)
                other.Hide();
            await wait.Task;
        }
        open = dialog;
        var done = closed = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        try
        {
            var showing = dialog.ShowAsync();
            shown?.Invoke();
            return await showing;
        }
        catch (Exception)
        {
            // A dialog opened outside this helper is up.
            return ContentDialogResult.None;
        }
        finally
        {
            if (open == dialog)
            {
                open = null;
                closed = null;
            }
            done.TrySetResult();
        }
    }

    /// <summary>Closes the dialog that is up, if any, as Cancel.</summary>
    public static void HideOpen() => open?.Hide();
}
