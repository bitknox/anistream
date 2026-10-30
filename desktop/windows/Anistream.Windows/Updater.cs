using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;

namespace Anistream.Windows;

/// <summary>
/// The app updating itself. Once the engine finds a newer release with an installer for this
/// machine, it downloads into a temporary folder and is checked against the size and SHA-256
/// the release lists; Restart to update then runs it silently, per user and without a prompt,
/// quits, and opens the new version from where this one ran. A fixture run goes through the same
/// steps and fetches and installs nothing.
/// </summary>
sealed class Updater
{
    readonly DispatcherQueue ui;
    readonly bool staged;
    readonly string dir = Path.Combine(Path.GetTempPath(), "anistream-update");
    readonly string marker;
    static readonly HttpClient http = new() { Timeout = Timeout.InfiniteTimeSpan };
    CancellationTokenSource? cancel;
    Release? release;
    bool tell;

    public Updater(DispatcherQueue ui, bool staged)
    {
        this.ui = ui;
        this.staged = staged;
        // Beside the download, not in a profile's folders: the copy that opens after the
        // installer reads it whichever profile it runs on.
        marker = Path.Combine(dir, "updating");
    }

    /// <summary>How the update stands.</summary>
    public UpdateState State { get; private set; } = UpdateState.Idle;

    /// <summary>The release on offer, while there is one.</summary>
    public Release? Release => release;

    /// <summary>This machine's installer in the release on offer, when it has one.</summary>
    public ReleaseAsset? Installer => release is null ? null : Updates.InstallerFor(release, RuntimeInformation.OSArchitecture);

    /// <summary>The version running: the one About shows.</summary>
    public static string Current => typeof(Updater).Assembly.GetName().Version?.ToString(3) ?? "0.0.0";

    /// <summary>Anything changed, on the UI thread.</summary>
    public event Action? Changed;

    /// <summary>A release the engine said to tell of is downloaded and ready.</summary>
    public event Action<string>? Ready;

    /// <summary>How the app quits for the installer.</summary>
    public Action? Quit { get; set; }

    /// <summary>What the engine's update check said.</summary>
    public void Heard(UpdateStatus status)
    {
        switch (status)
        {
            case UpdateStatus.Available a:
                release = a.Release;
                tell |= a.Notify;
                // A newer release than the one under way takes its place.
                if (State.Version != release.Version || State.Step == UpdateStep.Idle)
                    Download();
                break;
            case UpdateStatus.UpToDate:
                release = null;
                Stop();
                break;
        }
        Changed?.Invoke();
    }

    /// <summary>Downloads and checks this machine's installer, when the release has one.</summary>
    public async void Download()
    {
        if (release is not { } r || Installer is not { } asset || State.Step is UpdateStep.Installing)
            return;
        cancel?.Cancel();
        var mine = cancel = new CancellationTokenSource();
        Set(new UpdateState(UpdateStep.Downloading, r.Version, 0, asset.Size));
        UpdateState end;
        if (staged)
            end = await Rehearse(r.Version, asset, mine.Token);
        else
        {
            Tidy(asset.Name);
            end = await Task.Run(() => Updates.Fetch(http, asset, r.Version, dir,
                step => ui.TryEnqueue(() => { if (cancel == mine) Set(step); }), mine.Token));
        }
        if (cancel != mine || mine.IsCancellationRequested)
            return;
        Set(end);
        if (end.Step == UpdateStep.Failed)
            AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the update to {r.Version} failed: {end.Reason}");
        if (end.Step == UpdateStep.Ready && tell)
        {
            tell = false;
            Ready?.Invoke(r.Version);
        }
    }

    /// <summary>Stops a download under way.</summary>
    public void Stop()
    {
        if (State.Step is UpdateStep.Installing)
            return;
        cancel?.Cancel();
        cancel = null;
        Set(UpdateState.Idle);
    }

    /// <summary>
    /// Restart to update: the installer runs once the app has quit, with no pages or prompts, and
    /// the app opens again from where it ran, which is where the installer puts the new version.
    /// </summary>
    public async void Install()
    {
        if (State is not { Step: UpdateStep.Ready, Version: { } version } ready)
            return;
        Set(ready with { Step = UpdateStep.Installing });
        if (staged)
        {
            await Task.Delay(1500);
            Set(new UpdateState(UpdateStep.Failed, version, Reason: T.UpdatesWhyFixture));
            return;
        }
        try
        {
            Directory.CreateDirectory(dir);
            File.WriteAllText(marker, version);
            var app = Environment.ProcessPath ?? throw new Win32Exception("the app's own path is unknown");
            Process.Start(new ProcessStartInfo("cmd.exe", Updates.RelaunchCommand(ready.Path!, Path.Combine(dir, "install.log"), app,
                Updates.RelaunchArguments(Environment.GetCommandLineArgs().Skip(1))))
            {
                UseShellExecute = false,
                CreateNoWindow = true,
                WorkingDirectory = dir,
            });
        }
        catch (Exception e) when (e is Win32Exception or IOException or UnauthorizedAccessException)
        {
            SmallFiles.Delete(marker);
            Set(new UpdateState(UpdateStep.Failed, version, Reason: T.UpdatesWhyInstaller(e.Message)));
            return;
        }
        Quit?.Invoke();
    }

    /// <summary>
    /// After a restart to update: what to say of it, once. The installer's log stays in the
    /// temporary folder when the version did not change.
    /// </summary>
    public (bool Installed, string Version)? AfterRestart()
    {
        // A marker that can't be read or removed, or that says nothing a version can be read
        // from, is let go of: it must not come back on every launch.
        if (SmallFiles.Read(marker) is not { } version)
            return null;
        if (!SmallFiles.Delete(marker) || version.Length == 0)
            return null;
        try
        {
            var installed = !Updates.Older(Current, version);
            if (installed)
                Tidy(null);
            return (installed, version);
        }
        catch (Exception e)
        {
            AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the update marker said no version: {e.Message}");
            return null;
        }
    }

    void Set(UpdateState state)
    {
        State = state;
        Changed?.Invoke();
    }

    /// <summary>Removes what earlier updates left, apart from <paramref name="keep"/>.</summary>
    void Tidy(string? keep)
    {
        try
        {
            if (!Directory.Exists(dir))
                return;
            foreach (var file in Directory.EnumerateFiles(dir))
                if (keep is null || !Path.GetFileName(file).StartsWith(keep, StringComparison.Ordinal))
                    File.Delete(file);
        }
        catch (IOException)
        {
            // Left for the next update to clear.
        }
        catch (UnauthorizedAccessException)
        {
        }
    }

    /// <summary>A fixture run's download: the steps at a believable pace, and nothing fetched.</summary>
    async Task<UpdateState> Rehearse(string version, ReleaseAsset asset, CancellationToken token)
    {
        try
        {
            for (var step = 0; step <= 20; step++)
            {
                Set(new UpdateState(UpdateStep.Downloading, version, asset.Size * (ulong)step / 20, asset.Size));
                await Task.Delay(150, token);
            }
            Set(new UpdateState(UpdateStep.Verifying, version, asset.Size, asset.Size));
            await Task.Delay(600, token);
            return new UpdateState(UpdateStep.Ready, version, asset.Size, asset.Size, Path: Path.Combine(dir, asset.Name));
        }
        catch (TaskCanceledException)
        {
            return UpdateState.Idle;
        }
    }
}
