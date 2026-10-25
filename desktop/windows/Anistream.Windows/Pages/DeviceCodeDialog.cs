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
/// way there, as the Mac's sheet has it, with how long the code has left. It stays until the
/// engine says the sign-in ended, or the viewer closes it; the engine keeps waiting either way.
/// Once the code expires, Copy code becomes Get a new code, which asks the engine for another.
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
    readonly TextBlock expiry = Ui.Text("", "HazeCaptionStyle");
    readonly ProgressRing ring = new() { Width = 16, Height = 16, IsActive = true };
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer tick;
    DeviceCodePrompt prompt;

    /// <summary>Whether the code has run out: the dialog then offers a new one.</summary>
    bool Expired => prompt.ExpiresAt is { } at && at <= DateTimeOffset.UtcNow.ToUnixTimeSeconds();

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
        var wait = new Grid { ColumnSpacing = 10 };
        wait.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        wait.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        wait.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        wait.Children.Add(ring);
        Grid.SetColumn(waiting, 1);
        wait.Children.Add(waiting);
        // The countdown in tabular figures, so the seconds tick without the line shifting.
        Microsoft.UI.Xaml.Documents.Typography.SetNumeralAlignment(expiry, FontNumeralAlignment.Tabular);
        Grid.SetColumn(expiry, 2);
        wait.Children.Add(expiry);
        body.Children.Add(wait);
        tick = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread().CreateTimer();
        tick.Interval = TimeSpan.FromSeconds(1);
        tick.Tick += (_, _) => ShowExpiry();
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
            if (Expired)
            {
                // A new code replaces this one, through the engine's DeviceCode.
                Ctx.Current.Send(new Command.Connect(this.prompt.Tracker));
                dialog.IsSecondaryButtonEnabled = false;
                return;
            }
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
        dialog.IsSecondaryButtonEnabled = true;
        ShowExpiry();
    }

    /// <summary>"Expires in 15 min" under the code, by the second in the last minute, then Code expired and Get a new code.</summary>
    void ShowExpiry()
    {
        if (prompt.ExpiresAt is not { } at)
        {
            expiry.Text = "";
            tick.Stop();
            return;
        }
        var left = at - DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        expiry.Text = AnistreamFfiMethods.DeviceCodeExpiry(left);
        var expired = left <= 0;
        code.Opacity = expired ? 0.4 : 1;
        ring.IsActive = !expired;
        ring.Visibility = expired ? Visibility.Collapsed : Visibility.Visible;
        if (expired)
        {
            tick.Stop();
            if (dialog.SecondaryButtonText == "Copy code" || dialog.SecondaryButtonText == "Copied")
                dialog.SecondaryButtonText = "Get a new code";
        }
        else if (!tick.IsRunning)
            tick.Start();
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
            tick.Stop();
            if (open == this)
                open = null;
        }
    }
}
