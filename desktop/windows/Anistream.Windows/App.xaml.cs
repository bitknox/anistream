using Microsoft.UI.Xaml;

namespace Anistream.Windows;

public partial class App : Application
{
    MainWindow? window;
    StartFailedWindow? failed;

    public App()
    {
        InitializeComponent();
        // What reaches here goes to the engine's log, and to a file for a run with no engine yet.
        // One handler's mistake (a button's, a page's) is not worth the window: the app goes on.
        // Only what leaves the process in no state to go on ends it.
        UnhandledException += (_, e) =>
        {
            e.Handled = e.Exception is not (OutOfMemoryException or InsufficientExecutionStackException or System.Runtime.InteropServices.SEHException);
            try
            {
                Anistream.Engine.Ffi.AnistreamFfiMethods.LogLine(Anistream.Engine.Ffi.FfiLogLevel.Error, "windows",
                    $"{(e.Handled ? "went on after" : "stopped on")} {e.Exception}");
            }
            catch (Exception)
            {
                // No engine log yet.
            }
            try
            {
                var dir = Path.Combine(Path.GetTempPath(), "anistream-windows");
                Directory.CreateDirectory(dir);
                File.AppendAllText(Path.Combine(dir, "crash.txt"), $"{DateTime.Now:O} {e.Exception}\n");
            }
            catch (Exception)
            {
                // The crash file is a courtesy: it must not throw over the error it records.
            }
        };
    }

    static App()
    {
        // A task nobody waited on that failed: logged, never the end of the app.
        TaskScheduler.UnobservedTaskException += (_, e) =>
        {
            e.SetObserved();
            try
            {
                Anistream.Engine.Ffi.AnistreamFfiMethods.LogLine(Anistream.Engine.Ffi.FfiLogLevel.Warn, "windows", $"a task failed unseen: {e.Exception.InnerException?.Message}");
            }
            catch (Exception)
            {
                // No engine log yet.
            }
        };
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        var options = Options.Parse(Environment.GetCommandLineArgs().Skip(1));
        StringResources.Install(options.Language, options.Fixture);
        try
        {
            window = new MainWindow(options);
        }
        catch (Anistream.Engine.Ffi.FfiException e)
        {
            // The engine refused to start (a folder, config.toml, the database): say why, in a
            // window of its own, rather than end before any window shows.
            Anistream.Engine.Ffi.AnistreamFfiMethods.LogLine(Anistream.Engine.Ffi.FfiLogLevel.Error, "windows", $"the engine did not start: {e.Message}");
            failed = new StartFailedWindow(StartFailedWindow.Reason(e), Anistream.App.AppDirs.User().Config);
            failed.Activate();
            return;
        }
        window.Activate();
    }
}
