using System.Globalization;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>Where an update of the app stands, once the engine has found a release.</summary>
public enum UpdateStep
{
    /// <summary>Nothing under way.</summary>
    Idle,
    /// <summary>The installer is downloading.</summary>
    Downloading,
    /// <summary>Its size and SHA-256 are being checked.</summary>
    Verifying,
    /// <summary>Downloaded and checked: Restart to update installs it.</summary>
    Ready,
    /// <summary>The installer runs and the app quits; it opens again once installed.</summary>
    Installing,
    /// <summary>It stopped, and <see cref="UpdateState.Reason"/> says why.</summary>
    Failed,
}

/// <summary>An update's step, for the version it is for, with how far a download has come.</summary>
public sealed record UpdateState(UpdateStep Step, string? Version = null, ulong Received = 0, ulong Expected = 0, string? Reason = null, string? Path = null)
{
    public static readonly UpdateState Idle = new(UpdateStep.Idle);

    /// <summary>How far along, from nought to one, while that is known.</summary>
    public double? Fraction => Step switch
    {
        UpdateStep.Downloading when Expected > 0 => Math.Min(1, (double)Received / Expected),
        UpdateStep.Verifying or UpdateStep.Ready or UpdateStep.Installing => 1,
        _ => null,
    };

    /// <summary>Whether an update is on its way: downloading, being checked, or installing.</summary>
    public bool Busy => Step is UpdateStep.Downloading or UpdateStep.Verifying or UpdateStep.Installing;
}

/// <summary>
/// The Windows app's updates: the engine names each release's per-user installers, one for x64
/// and one for Arm64, each with its SHA-256; the app downloads this machine's, checks it, and on
/// Restart to update runs it silently and opens again.
/// </summary>
public static class Updates
{
    /// <summary>
    /// The installer for <paramref name="arch"/>: Arm64's on an Arm64 machine, or x64's there when
    /// a release has no Arm64 one, as Windows 11 runs x64 apps on Arm; x64's on x64.
    /// </summary>
    public static ReleaseAsset? InstallerFor(Release release, Architecture arch) => arch switch
    {
        Architecture.Arm64 => release.WindowsArm64 ?? release.WindowsX64,
        Architecture.X64 => release.WindowsX64,
        _ => null,
    };

    /// <summary>How long a download may go without a byte before it counts as lost.</summary>
    public static readonly TimeSpan Stall = TimeSpan.FromSeconds(60);

    /// <summary>
    /// Downloads <paramref name="asset"/> into <paramref name="dir"/> and checks it, saying how far
    /// it has come. One already there and whole is kept; one that does not match is deleted.
    /// </summary>
    public static async Task<UpdateState> Fetch(HttpClient http, ReleaseAsset asset, string version, string dir,
        Action<UpdateState> progress, CancellationToken cancel, TimeSpan? stall = null)
    {
        var path = System.IO.Path.Combine(dir, asset.Name);
        // A connection that stops sending without closing would wait for ever: a minute with no
        // bytes ends the download as a lost connection.
        var quiet = stall ?? Stall;
        using var stalled = CancellationTokenSource.CreateLinkedTokenSource(cancel);
        stalled.CancelAfter(quiet);
        try
        {
            Directory.CreateDirectory(dir);
            if (File.Exists(path) && await Check(path, asset, cancel) is null)
                return new UpdateState(UpdateStep.Ready, version, asset.Size, asset.Size, Path: path);
            var partial = path + ".partial";
            using (var response = await http.GetAsync(asset.Url, HttpCompletionOption.ResponseHeadersRead, stalled.Token))
            {
                if (!response.IsSuccessStatusCode)
                    return Failed(version, T.UpdatesWhyServer((int)response.StatusCode));
                stalled.CancelAfter(quiet);
                await using var body = await response.Content.ReadAsStreamAsync(stalled.Token);
                await using var file = File.Create(partial);
                var buffer = new byte[81920];
                ulong received = 0;
                var told = DateTime.MinValue;
                int read;
                while ((read = await body.ReadAsync(buffer, stalled.Token)) > 0)
                {
                    stalled.CancelAfter(quiet);
                    await file.WriteAsync(buffer.AsMemory(0, read), cancel);
                    received += (ulong)read;
                    if (received > asset.Size)
                        break;
                    if (DateTime.UtcNow - told > TimeSpan.FromMilliseconds(100))
                    {
                        told = DateTime.UtcNow;
                        progress(new UpdateState(UpdateStep.Downloading, version, received, asset.Size));
                    }
                }
            }
            progress(new UpdateState(UpdateStep.Verifying, version, asset.Size, asset.Size));
            File.Move(partial, path, overwrite: true);
            if (await Check(path, asset, cancel) is { } wrong)
            {
                File.Delete(path);
                return Failed(version, wrong);
            }
            return new UpdateState(UpdateStep.Ready, version, asset.Size, asset.Size, Path: path);
        }
        catch (OperationCanceledException) when (cancel.IsCancellationRequested)
        {
            return UpdateState.Idle;
        }
        catch (OperationCanceledException) when (stalled.IsCancellationRequested)
        {
            SmallFiles.Delete(path + ".partial");
            return Failed(version, T.UpdatesWhyConnection);
        }
        catch (HttpRequestException)
        {
            return Failed(version, T.UpdatesWhyConnection);
        }
        catch (IOException e)
        {
            return Failed(version, T.UpdatesWhySave(e.Message));
        }
        catch (UnauthorizedAccessException e)
        {
            return Failed(version, T.UpdatesWhySave(e.Message));
        }
    }

    /// <summary>What is wrong with the file at <paramref name="path"/> for <paramref name="asset"/>, or null when it matches.</summary>
    public static async Task<string?> Check(string path, ReleaseAsset asset, CancellationToken cancel = default)
    {
        var size = (ulong)new FileInfo(path).Length;
        if (size != asset.Size)
            return T.UpdatesWhySize(Wording.Bytes(size), Wording.Bytes(asset.Size));
        await using var file = File.OpenRead(path);
        var hash = Convert.ToHexStringLower(await SHA256.HashDataAsync(file, cancel));
        return hash == asset.Sha256.ToLowerInvariant() ? null : T.UpdatesWhyChecksum;
    }

    static UpdateState Failed(string version, string reason) => new(UpdateStep.Failed, version, Reason: reason);

    /// <summary>
    /// Inno Setup's switches for an update nobody watches: no pages, no message boxes, no restart
    /// of Windows, the running app closed, and a log beside the download.
    /// </summary>
    public static string InstallerSwitches(string log) =>
        $"/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /CLOSEAPPLICATIONS /LOG=\"{log}\"";

    /// <summary>
    /// What cmd.exe runs as the app quits: two seconds for it to go, the installer, waited for,
    /// then the app again from where it was, which is where the installer puts the new version,
    /// with <paramref name="arguments"/>.
    /// </summary>
    public static string RelaunchCommand(string installer, string log, string app, IEnumerable<string>? arguments = null)
    {
        var again = string.Concat((arguments ?? []).Select(a => " " + (a.Contains(' ') ? $"\"{a}\"" : a)));
        return $"/d /c ping -n 3 127.0.0.1 >nul & start \"\" /wait \"{installer}\" {InstallerSwitches(log)} & start \"\" \"{app}\"{again}";
    }

    /// <summary>
    /// The arguments the app opens again with after an update: the ones it started with, less a
    /// link it was opened by and the testing switches that set the update going.
    /// </summary>
    public static IEnumerable<string> RelaunchArguments(IEnumerable<string> started) => started.Where(a =>
        !a.StartsWith("anistream:", StringComparison.OrdinalIgnoreCase) &&
        !a.StartsWith("--show=", StringComparison.Ordinal) &&
        !a.StartsWith("--update-from=", StringComparison.Ordinal));

    /// <summary>Whether the version <paramref name="a"/> is older than <paramref name="b"/>, number by number: 2.0.10 is newer than 2.0.9.</summary>
    public static bool Older(string a, string b)
    {
        static int[] Parts(string v) => v.Split('.', '-')
            .Select(p => int.TryParse(p, NumberStyles.None, CultureInfo.InvariantCulture, out var n) ? n : 0).ToArray();
        var (x, y) = (Parts(a), Parts(b));
        for (var i = 0; i < Math.Max(x.Length, y.Length); i++)
        {
            var (p, q) = (i < x.Length ? x[i] : 0, i < y.Length ? y[i] : 0);
            if (p != q)
                return p < q;
        }
        return false;
    }

    /// <summary>The line under the version in About, for the check and the update together.</summary>
    public static string Line(UpdateStatus? status, UpdateState state, bool offline, bool installsHere)
    {
        switch (state.Step)
        {
            case UpdateStep.Downloading:
                return state.Expected > 0
                    ? T.UpdatesDownloadingSize(state.Version ?? "", Wording.Bytes(state.Received), Wording.Bytes(state.Expected))
                    : T.UpdatesDownloading(state.Version ?? "");
            case UpdateStep.Verifying:
                return T.UpdatesCheckingDownload(state.Version ?? "");
            case UpdateStep.Ready:
                return T.UpdatesReady(state.Version ?? "");
            case UpdateStep.Installing:
                return T.UpdatesInstalling(state.Version ?? "");
            case UpdateStep.Failed:
                return T.UpdatesStopped(state.Version ?? "", state.Reason ?? "");
        }
        return status switch
        {
            UpdateStatus.Checking => T.UpdatesChecking,
            UpdateStatus.UpToDate => T.UpdatesUpToDate,
            UpdateStatus.Available a when installsHere => T.UpdatesVersionOut(a.Release.Version),
            UpdateStatus.Available a => T.UpdatesVersionOutPage(a.Release.Version),
            // The engine's reason is for the log: the line says what to do.
            UpdateStatus.Failed => T.UpdatesNotCheckedBecause(T.StateCheckConnection),
            _ when offline => T.UpdatesChecksOnline,
            _ => T.UpdatesNotChecked,
        };
    }
}
