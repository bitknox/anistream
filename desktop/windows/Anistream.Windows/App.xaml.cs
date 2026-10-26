using Microsoft.UI.Xaml;

namespace Anistream.Windows;

public partial class App : Application
{
    MainWindow? window;

    public App()
    {
        InitializeComponent();
        // What kills the app goes to the engine's log, and to a file beside it for a run with no
        // engine yet.
        UnhandledException += (_, e) =>
        {
            var dir = Path.Combine(Path.GetTempPath(), "anistream-windows");
            Directory.CreateDirectory(dir);
            File.AppendAllText(Path.Combine(dir, "crash.txt"), $"{DateTime.Now:O} {e.Exception}\n");
        };
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        StringResources.Install();
        window = new MainWindow(Options.Parse(Environment.GetCommandLineArgs().Skip(1)));
        window.Activate();
    }
}
