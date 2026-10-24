namespace Anistream.PlayerSpike;

/// <summary>
/// The spike's command line:
/// <c>Anistream.PlayerSpike.exe FILE [--output=sdr|pq|hdr|scrgb] [--peak=NITS] [--vo=gpu-next|gpu]
/// [--report=PATH] [--pause-at=SECONDS] [--switch-at=SECONDS] [--quit-at=SECONDS]
/// [--resize-at=SECONDS] [--set=OPTION=VALUE]...</c>, where each <c>--set</c> is one more mpv option.
/// </summary>
/// <param name="Output">
/// The swapchain mpv makes: <c>sdr</c> leaves mpv's defaults (libplacebo then picks 10-bit
/// sRGB), <c>pq</c> asks mpv's d3d11 layer for a 10-bit HDR10 swapchain and <c>scrgb</c>
/// for FP16 linear, and <c>hdr</c> is <c>pq</c> with <c>target-colorspace-hint</c> on.
/// </param>
/// <param name="SwitchAt">When to flip <c>target-colorspace-hint</c> while playing, which libplacebo
/// applies to the same swapchain.</param>
internal sealed record SpikeOptions(
    string? File,
    string Output,
    double? Peak,
    string Vo,
    string? Report,
    double? PauseAt,
    double? SwitchAt,
    double? QuitAt,
    double? ResizeAt,
    IReadOnlyList<(string Name, string Value)> Set)
{
    public static SpikeOptions Parse(IEnumerable<string> args)
    {
        string? file = null, report = null;
        string output = "sdr", vo = "gpu-next";
        double? peak = null, pauseAt = null, switchAt = null, quitAt = null, resizeAt = null;
        var set = new List<(string, string)>();
        foreach (var arg in args)
        {
            var (key, value) = arg.StartsWith("--") && arg.Contains('=')
                ? (arg[2..arg.IndexOf('=')], arg[(arg.IndexOf('=') + 1)..])
                : (null, arg);
            switch (key)
            {
                case null: file = value; break;
                case "output": output = value; break;
                case "peak": peak = double.Parse(value); break;
                case "vo": vo = value; break;
                case "report": report = value; break;
                case "pause-at": pauseAt = double.Parse(value); break;
                case "switch-at": switchAt = double.Parse(value); break;
                case "quit-at": quitAt = double.Parse(value); break;
                case "resize-at": resizeAt = double.Parse(value); break;
                case "set" when value.Contains('='):
                    set.Add((value[..value.IndexOf('=')], value[(value.IndexOf('=') + 1)..]));
                    break;
            }
        }
        return new(file, output, peak, vo, report, pauseAt, switchAt, quitAt, resizeAt, set);
    }
}
