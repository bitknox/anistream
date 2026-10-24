using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Anistream.Windows.Pages;

/// <summary>
/// A code to type on another device, for Simkl or Trakt: the code large, where to type it, and a
/// way there, as the Mac's sheet has it. It stays until the engine says the sign-in ended, or the
/// viewer closes it; the engine keeps waiting either way.
/// </summary>
sealed class DeviceCodeDialog
{
    static DeviceCodeDialog? open;

    readonly ContentDialog dialog;
    readonly TextBlock line = Ui.Text("", "HazeBodyStyle");
    readonly TextBlock code = new()
    {
        FontFamily = new FontFamily("Cascadia Mono, Consolas"),
        FontSize = 34,
        FontWeight = FontWeights.SemiBold,
        CharacterSpacing = 200,
        IsTextSelectionEnabled = true,
        HorizontalAlignment = HorizontalAlignment.Center,
    };
    readonly TextBlock waiting = Ui.Text("", "HazeCaptionStyle");
    DeviceCodePrompt prompt;

    /// <summary>The engine's <c>DeviceCode</c>: a prompt opens or refreshes the dialog, none closes it.</summary>
    public static void Heard(XamlRoot root, DeviceCodePrompt? prompt)
    {
        if (prompt is null)
        {
            open?.dialog.Hide();
            open = null;
            return;
        }
        if (open is { } shown)
        {
            shown.Fill(prompt);
            return;
        }
        open = new DeviceCodeDialog(root, prompt);
        _ = open.ShowAsync();
    }

    DeviceCodeDialog(XamlRoot root, DeviceCodePrompt prompt)
    {
        this.prompt = prompt;
        var body = new StackPanel { Spacing = 16, MinWidth = 380 };
        body.Children.Add(line);
        body.Children.Add(new Border
        {
            Child = code,
            Padding = new Thickness(0, 14, 0, 14),
            CornerRadius = new CornerRadius(10),
            Background = Ui.Res<Brush>("GroundBrush"),
        });
        var wait = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10 };
        wait.Children.Add(new ProgressRing { Width = 16, Height = 16, IsActive = true });
        wait.Children.Add(waiting);
        body.Children.Add(wait);
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Content = body,
            SecondaryButtonText = "Copy code",
            CloseButtonText = "Close",
            DefaultButton = ContentDialogButton.Primary,
        };
        dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        // Neither button ends the sign-in, so neither closes the dialog.
        dialog.PrimaryButtonClick += (sender, e) =>
        {
            e.Cancel = true;
            if (Uri.TryCreate(this.prompt.Url, UriKind.Absolute, out var url))
                _ = global::Windows.System.Launcher.LaunchUriAsync(url);
        };
        dialog.SecondaryButtonClick += (sender, e) =>
        {
            e.Cancel = true;
            var data = new global::Windows.ApplicationModel.DataTransfer.DataPackage();
            data.SetText(this.prompt.Code);
            global::Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(data);
            dialog.SecondaryButtonText = "Copied";
        };
        Fill(prompt);
    }

    void Fill(DeviceCodePrompt now)
    {
        prompt = now;
        var name = AnistreamFfiMethods.TrackerName(now.Tracker);
        var host = Uri.TryCreate(now.Url, UriKind.Absolute, out var url) ? url.Host : now.Url;
        dialog.Title = $"Sign in to {name}";
        line.Text = $"Go to {host} and enter this code. anistream signs in once you have.";
        code.Text = now.Code;
        AutomationProperties.SetName(code, "Code " + string.Join(' ', now.Code.ToCharArray()));
        waiting.Text = $"Waiting for {name}";
        dialog.PrimaryButtonText = $"Open {host}";
        dialog.SecondaryButtonText = "Copy code";
    }

    async Task ShowAsync()
    {
        try
        {
            await dialog.ShowAsync();
        }
        catch (Exception)
        {
            // Another dialog is up: the engine has opened the page itself, and the code is in its toast.
        }
        finally
        {
            if (open == this)
                open = null;
        }
    }
}
