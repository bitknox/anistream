using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows.Pages;

/// <summary>
/// Kitsu's sign-in: an email or user name and a password, sent once for a token
/// (<c>ConnectKitsu</c>). Kitsu has no browser sign-in for apps, so this is the only way in.
/// </summary>
static class KitsuDialog
{
    public static async void Show(Ctx ctx, XamlRoot root)
    {
        var kitsu = AnistreamFfiMethods.TrackerName("kitsu");
        var user = new TextBox { Header = T.AccountsUserOrEmail, InputScope = Scope(Microsoft.UI.Xaml.Input.InputScopeNameValue.EmailSmtpAddress) };
        var password = new PasswordBox { Header = T.AccountsPassword };
        var body = new StackPanel { Spacing = 12, MinWidth = 340 };
        body.Children.Add(Ui.Text(T.AccountsFormDetail(kitsu), "BodyTextBlockStyle"));
        body.Children.Add(user);
        body.Children.Add(password);
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.AccountsFormTitle(kitsu),
            Content = body,
            PrimaryButtonText = T.ActionSignIn,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
            IsPrimaryButtonEnabled = false,
        };
        void Complete() => dialog.IsPrimaryButtonEnabled = user.Text.Trim().Length > 0 && password.Password.Length > 0;
        user.TextChanged += (_, _) => Complete();
        password.PasswordChanged += (_, _) => Complete();
        password.KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.Enter && dialog.IsPrimaryButtonEnabled)
            {
                e.Handled = true;
                Send(ctx, user, password);
                dialog.Hide();
            }
        };
        if (await Dialogs.Show(dialog, () => user.Focus(FocusState.Programmatic)) == ContentDialogResult.Primary)
            Send(ctx, user, password);
        password.Password = "";
    }

    static void Send(Ctx ctx, TextBox user, PasswordBox password)
    {
        if (password.Password.Length == 0)
            return;
        ctx.Send(new Command.ConnectKitsu(user.Text.Trim(), password.Password));
        password.Password = "";
    }

    static Microsoft.UI.Xaml.Input.InputScope Scope(Microsoft.UI.Xaml.Input.InputScopeNameValue name)
    {
        var scope = new Microsoft.UI.Xaml.Input.InputScope();
        scope.Names.Add(new Microsoft.UI.Xaml.Input.InputScopeName(name));
        return scope;
    }
}
