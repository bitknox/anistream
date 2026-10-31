using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// The words for media servers, folders and licensed services. The engine sends states and
/// failures as data, so every line here is this app's own string.
/// </summary>
public static class Lawful
{
    /// <summary>A server error as a sentence. <paramref name="address"/> is what was typed, for an answer that names none.</summary>
    public static string Failure(ServerFailure failure, string address) => failure switch
    {
        ServerFailure.Unreachable u => T.ServersErrorUnreachable(u.Address is { Length: > 0 } ? u.Address : address),
        ServerFailure.NotJellyfin => T.ServersErrorNotJellyfin,
        ServerFailure.TooOld old => T.ServersErrorTooOld(old.Version),
        ServerFailure.Credentials => T.ServersErrorCredentials,
        ServerFailure.QuickConnectOff => T.ServersErrorQcOff,
        ServerFailure.QuickConnectExpired => T.ServersQcExpired,
        ServerFailure.PlainHttp => T.ServersErrorPlainHttp,
        ServerFailure.Certificate c => T.ServersCertBody(c.Host),
        ServerFailure.SignedOut => T.ServersRowSignedOut,
        ServerFailure.BadAddress => T.ServersErrorBadAddress,
        _ => T.ServersErrorOther,
    };

    /// <summary>The line under a server's name: signed out, matching while series are still unmatched, or the summary.</summary>
    public static string ServerLine(ServerRow row)
    {
        if (row.SignedOut)
            return T.ServersRowSignedOut;
        return T.ServersRowSummary(row.Libraries.Count(l => l.Ticked), row.Series);
    }

    /// <summary>Matching progress while some of a server's series are not matched yet, or null.</summary>
    public static string? ServerMatching(ServerRow row) =>
        !row.SignedOut && row.Series > 0 && row.Matched == 0 ? T.ServersRowMatching(row.Matched, row.Series) : null;

    /// <summary>A library's kind in its line.</summary>
    public static string LibraryKind(string? kind) => kind switch
    {
        "tvshows" => T.ServersKindShows,
        "movies" => T.ServersKindMovies,
        _ => T.ServersKindOther,
    };

    /// <summary>A library's line in the Choose libraries step: its kind and series.</summary>
    public static string LibraryLine(LibraryChoice library, int series) => T.ServersLibraryLine(LibraryKind(library.Kind), series);

    public static int NeedsLook(LocalView view) => view.Groups.Count(g => g.State == GroupState.NeedsLook);

    /// <summary>
    /// A folder's line: the drive being away, the scan counting, or its summary. The engine does not
    /// say which group sits in which folder, so with several folders each line counts only files.
    /// </summary>
    public static string FolderLine(FolderRow row, LocalView view)
    {
        if (!row.Available)
            return T.LibraryRowDriveOffline;
        if (view.Scanning)
            return T.LibraryRowScanning(row.Files);
        if (view.Folders.Length != 1)
            return T.LibraryFiles(row.Files);
        var summary = T.LibraryRowSummary(T.LibraryEpisodes(row.Files), T.LibrarySeries(view.Groups.Length));
        return NeedsLook(view) is > 0 and var look ? $"{summary} · {T.LibraryRowNeedsLook(look)}" : summary;
    }

    /// <summary>Why a file group is not matched.</summary>
    public static string Reason(NotMatchedReason? reason) => reason switch
    {
        NotMatchedReason.Offline => T.LibraryRowDriveOffline,
        _ => T.LibraryReasonNoTitle,
    };

    /// <summary>The Licensed services row on Your sources.</summary>
    public static string LicensedLine(LicensedView view) =>
        view.Settings.Services.Length == 0
            ? T.LicensedRowNone
            : view.Region is { Length: > 0 } region
                ? T.LicensedRowLine(string.Join(", ", view.Settings.Services), RegionName(region))
                : string.Join(", ", view.Settings.Services);

    /// <summary>A country code as the system names it, or the code when it knows none.</summary>
    public static string RegionName(string code)
    {
        try
        {
            return new System.Globalization.RegionInfo(code).DisplayName;
        }
        catch (ArgumentException)
        {
            return code;
        }
    }
}
