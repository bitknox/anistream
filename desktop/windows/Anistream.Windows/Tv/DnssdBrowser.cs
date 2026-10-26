using Anistream.Engine.Ffi;
using Windows.Devices.Enumeration;

namespace Anistream.Windows.Tv;

/// <summary>
/// The browse for TVs: Windows' DNS-SD, through a device watcher on <c>_anistream._tcp</c> in
/// <c>local</c>, as docs/remote-protocol.md has the Windows client find TVs. Each TV found, and
/// found again with new details, goes to the engine with its addresses, port and TXT record; the
/// engine decides what the picker lists. Started and stopped on the engine's threads.
/// </summary>
sealed class DnssdBrowser : FfiTvBrowser
{
    const string Query =
        "System.Devices.AepService.ProtocolId:=\"{4526e8c1-8aac-4153-9b16-55e86ada0e54}\" AND " +
        "System.Devices.Dnssd.Domain:=\"local\" AND System.Devices.Dnssd.ServiceName:=\"_anistream._tcp\"";

    const string InstanceName = "System.Devices.Dnssd.InstanceName";
    const string PortNumber = "System.Devices.Dnssd.PortNumber";
    const string TextAttributes = "System.Devices.Dnssd.TextAttributes";
    const string IpAddress = "System.Devices.IpAddress";

    static readonly string[] Properties = [InstanceName, PortNumber, TextAttributes, IpAddress];

    readonly object gate = new();
    readonly Dictionary<string, DeviceInformation> seen = [];
    DeviceWatcher? watcher;
    FfiDiscoverySink? sink;

    public void Start(FfiDiscoverySink to)
    {
        lock (gate)
        {
            StopLocked();
            sink = to;
            try
            {
                var next = DeviceInformation.CreateWatcher(Query, Properties, DeviceInformationKind.AssociationEndpointService);
                next.Added += Added;
                next.Updated += Updated;
                next.Removed += Removed;
                watcher = next;
                next.Start();
            }
            catch (Exception e)
            {
                // No DNS-SD on this machine: the picker says no TV answered, and an address still works.
                AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "tv", $"the TV browse did not start: {e.Message}");
                watcher = null;
            }
        }
    }

    public void Stop()
    {
        lock (gate)
            StopLocked();
    }

    void StopLocked()
    {
        if (watcher is { } running)
        {
            running.Added -= Added;
            running.Updated -= Updated;
            running.Removed -= Removed;
            if (running.Status is DeviceWatcherStatus.Started or DeviceWatcherStatus.EnumerationCompleted)
                running.Stop();
        }
        watcher = null;
        sink = null;
        seen.Clear();
    }

    void Added(DeviceWatcher from, DeviceInformation device)
    {
        lock (gate)
        {
            if (from != watcher)
                return;
            seen[device.Id] = device;
            Report(device);
        }
    }

    void Updated(DeviceWatcher from, DeviceInformationUpdate update)
    {
        lock (gate)
        {
            if (from != watcher || !seen.TryGetValue(update.Id, out var device))
                return;
            device.Update(update);
            Report(device);
        }
    }

    void Removed(DeviceWatcher from, DeviceInformationUpdate update)
    {
        lock (gate)
        {
            if (from != watcher || !seen.Remove(update.Id, out var device))
                return;
            if (Instance(device) is { } instance)
                sink?.Lost(instance);
        }
    }

    void Report(DeviceInformation device)
    {
        if (sink is null || Instance(device) is not { } instance)
            return;
        var props = device.Properties;
        if (!props.TryGetValue(PortNumber, out var portValue) || portValue is null)
            return;
        var port = Convert.ToUInt16(portValue);
        var addresses = props.TryGetValue(IpAddress, out var ips) && ips is string[] list ? list : [];
        // An instance not yet resolved to an address is reported once it is.
        if (addresses.Length == 0)
            return;
        var txt = props.TryGetValue(TextAttributes, out var attributes) && attributes is string[] pairs
            ? pairs.Select(Pair).OfType<FfiTxtPair>().ToArray()
            : [];
        sink.Found(instance, addresses, port, txt);
    }

    static string? Instance(DeviceInformation device) =>
        device.Properties.TryGetValue(InstanceName, out var name) && name is string { Length: > 0 } text ? text : null;

    /// <summary>A TXT attribute as Windows gives it, <c>key=value</c>.</summary>
    static FfiTxtPair? Pair(string attribute)
    {
        var at = attribute.IndexOf('=');
        return at <= 0 ? (attribute.Length > 0 ? new FfiTxtPair(attribute, "") : null) : new FfiTxtPair(attribute[..at], attribute[(at + 1)..]);
    }
}
