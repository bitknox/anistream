using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Windows.Networking.Connectivity;

namespace Anistream.Windows;

/// <summary>
/// Windows' network path, said to the engine each time it changes (<c>NetworkPath</c>), the way
/// NWPathMonitor feeds it on Apple. The engine answers with one app-wide <c>Connectivity</c>
/// state the pages render. A metered connection counts as cellular, a restricted or over-limit
/// one as constrained, and no profile with internet access as offline.
/// </summary>
sealed class NetworkWatch : IDisposable
{
    readonly Action<Command> send;
    readonly DispatcherQueue ui;
    readonly bool forceOffline;
    (bool, bool, bool)? last;

    public NetworkWatch(Action<Command> send, DispatcherQueue ui, bool forceOffline)
    {
        this.send = send;
        this.ui = ui;
        this.forceOffline = forceOffline;
        NetworkInformation.NetworkStatusChanged += Changed;
        Say();
    }

    void Changed(object sender) => ui.TryEnqueue(Say);

    void Say()
    {
        (bool Cellular, bool Constrained, bool Offline) path = forceOffline ? (false, false, true) : Read();
        if (path == last)
            return;
        last = path;
        send(new Command.NetworkPath(path.Cellular, path.Constrained, path.Offline));
    }

    static (bool Cellular, bool Constrained, bool Offline) Read()
    {
        try
        {
            var profile = NetworkInformation.GetInternetConnectionProfile();
            if (profile is null || profile.GetNetworkConnectivityLevel() != NetworkConnectivityLevel.InternetAccess)
                return (false, false, true);
            var cost = profile.GetConnectionCost();
            var metered = cost.NetworkCostType is NetworkCostType.Fixed or NetworkCostType.Variable || profile.IsWwanConnectionProfile;
            var constrained = cost.BackgroundDataUsageRestricted || cost.OverDataLimit;
            return (metered, constrained, false);
        }
        catch (Exception)
        {
            // Windows could not say: assume a plain connection, as the engine does without a path.
            return (false, false, false);
        }
    }

    public void Dispose() => NetworkInformation.NetworkStatusChanged -= Changed;
}
