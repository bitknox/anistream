using System.Runtime.InteropServices;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows.Player;

/// <summary>
/// The DXGI and XAML interop the player needs, called through vtables: handing mpv's swapchain to
/// a SwapChainPanel, reading the swapchain's format, undoing XAML's DPI stretch, and reading what
/// each display output says about HDR.
/// </summary>
internal static unsafe partial class Dxgi
{
    // ISwapChainPanelNative, from microsoft.ui.xaml.media.dxinterop.h (the same IID as UWP's).
    private static readonly Guid IidSwapChainPanelNative = new("63aad0b8-7c24-40ff-85a8-640d944cc325");
    private static readonly Guid IidSwapChain2 = new("a8be2ac4-199f-4946-b331-79599fb98de7");
    private static readonly Guid IidFactory1 = new("770aae78-f26f-4dba-a829-253c83d1b387");
    private static readonly Guid IidOutput6 = new("068346e8-aaec-4b84-add7-137f513f77a1");

    [LibraryImport("dxgi.dll")]
    private static partial int CreateDXGIFactory1(in Guid riid, out nint factory);

    private static void* Slot(nint obj, int index) => (*(void***)obj)[index];

    private static void Release(nint obj)
    {
        if (obj != 0)
            ((delegate* unmanaged[Stdcall]<nint, uint>)Slot(obj, 2))(obj);
    }

    /// <summary>ISwapChainPanelNative::SetSwapChain, on the UI thread. Zero detaches.</summary>
    public static void SetSwapChain(SwapChainPanel panel, nint swapchain)
    {
        var inspectable = ((WinRT.IWinRTObject)panel).NativeObject.ThisPtr;
        Marshal.ThrowExceptionForHR(Marshal.QueryInterface(inspectable, in IidSwapChainPanelNative, out var native));
        try
        {
            var hr = ((delegate* unmanaged[Stdcall]<nint, nint, int>)Slot(native, 3))(native, swapchain);
            Marshal.ThrowExceptionForHR(hr);
        }
        finally { Release(native); }
    }

    /// <summary>IDXGISwapChain::GetDesc: the buffers' size and DXGI_FORMAT.</summary>
    public static (uint Width, uint Height, uint Format) Describe(nint swapchain)
    {
        var desc = stackalloc byte[128];
        var hr = ((delegate* unmanaged[Stdcall]<nint, byte*, int>)Slot(swapchain, 12))(swapchain, desc);
        Marshal.ThrowExceptionForHR(hr);
        return (*(uint*)desc, *(uint*)(desc + 4), *(uint*)(desc + 16));
    }

    /// <summary>
    /// IDXGISwapChain2::SetMatrixTransform with 1/scale: XAML shows a swapchain's pixels one per
    /// effective pixel, so a swapchain sized in physical pixels has to be scaled back down.
    /// </summary>
    public static void SetInverseScale(nint swapchain, float scaleX, float scaleY)
    {
        Marshal.ThrowExceptionForHR(Marshal.QueryInterface(swapchain, in IidSwapChain2, out var sc2));
        try
        {
            var matrix = stackalloc float[6] { 1f / scaleX, 0, 0, 1f / scaleY, 0, 0 };
            var hr = ((delegate* unmanaged[Stdcall]<nint, float*, int>)Slot(sc2, 34))(sc2, matrix);
            Marshal.ThrowExceptionForHR(hr);
        }
        finally { Release(sc2); }
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct OutputDesc1
    {
        public fixed char DeviceName[32];
        public int Left, Top, Right, Bottom;
        public int AttachedToDesktop;
        public int Rotation;
        public nint Monitor;
        public uint BitsPerColor;
        public int ColorSpace;
        public fixed float Primaries[8];
        public float MinLuminance, MaxLuminance, MaxFullFrameLuminance;
    }

    public readonly record struct Output(
        string Adapter, string Device, uint BitsPerColor, int ColorSpace,
        float MinLuminance, float MaxLuminance, float MaxFullFrameLuminance)
    {
        public bool Hdr => ColorSpace == 12; // DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020

        public override string ToString() =>
            $"{Device} on {Adapter}: {ColorSpaceName(ColorSpace)}, {BitsPerColor} bpc, " +
            $"{MinLuminance:0.###}-{MaxLuminance:0} nits (full frame {MaxFullFrameLuminance:0})";
    }

    public static string ColorSpaceName(int cs) => cs switch
    {
        0 => "sRGB (G22 P709)",
        1 => "scRGB (G10 P709)",
        12 => "HDR10 (G2084 P2020)",
        17 => "G22 P2020",
        _ => $"colour space {cs}",
    };

    public static string FormatName(uint format) => format switch
    {
        24 => "R10G10B10A2_UNORM",
        10 => "R16G16B16A16_FLOAT",
        28 => "R8G8B8A8_UNORM",
        87 => "B8G8R8A8_UNORM",
        _ => $"format {format}",
    };

    /// <summary>Every output of every adapter, with IDXGIOutput6::GetDesc1's HDR fields.</summary>
    public static List<Output> Outputs()
    {
        var list = new List<Output>();
        Marshal.ThrowExceptionForHR(CreateDXGIFactory1(in IidFactory1, out var factory));
        var adesc = stackalloc byte[512];
        try
        {
            for (uint a = 0; ; a++)
            {
                nint adapter;
                // IDXGIFactory1::EnumAdapters1
                if (((delegate* unmanaged[Stdcall]<nint, uint, nint*, int>)Slot(factory, 12))(factory, a, &adapter) < 0)
                    break;
                try
                {
                    // IDXGIAdapter1::GetDesc1: the description is the first 128 WCHARs.
                    ((delegate* unmanaged[Stdcall]<nint, byte*, int>)Slot(adapter, 10))(adapter, adesc);
                    var adapterName = new string((char*)adesc).TrimEnd('\0');
                    for (uint o = 0; ; o++)
                    {
                        nint output;
                        // IDXGIAdapter::EnumOutputs
                        if (((delegate* unmanaged[Stdcall]<nint, uint, nint*, int>)Slot(adapter, 7))(adapter, o, &output) < 0)
                            break;
                        try
                        {
                            if (Marshal.QueryInterface(output, in IidOutput6, out var output6) < 0)
                                continue;
                            try
                            {
                                OutputDesc1 d;
                                // IDXGIOutput6::GetDesc1
                                Marshal.ThrowExceptionForHR(((delegate* unmanaged[Stdcall]<nint, OutputDesc1*, int>)Slot(output6, 27))(output6, &d));
                                list.Add(new Output(adapterName, new string(d.DeviceName).TrimEnd('\0'), d.BitsPerColor,
                                    d.ColorSpace, d.MinLuminance, d.MaxLuminance, d.MaxFullFrameLuminance));
                            }
                            finally { Release(output6); }
                        }
                        finally { Release(output); }
                    }
                }
                finally { Release(adapter); }
            }
        }
        finally { Release(factory); }
        return list;
    }
}
