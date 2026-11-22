// P/Invoke declarations of the FerroChart C ABI, mirroring
// crates/ferrochart-ffi/include/ferrochart.h. Structs are blittable and
// declared field by field in the header's order; each starts with a `Size`
// the caller sets, so a newer library still reads an older struct.

using System.Reflection;
using System.Runtime.InteropServices;

namespace FerroChart.Native;

public static class Status
{
    public const int Ok = 0;
    public const int ErrNull = -1;
    public const int ErrInvalidArgument = -2;
    public const int ErrUnknownId = -3;
    public const int ErrUtf8 = -4;
    public const int ErrRender = -5;
    public const int ErrPanic = -6;
    public const int ErrBufferTooSmall = -7;
}

public static class Const
{
    public const ulong MainPane = ulong.MaxValue;
    public const uint ThemeLight = 0, ThemeDark = 1;
    public const long PlacementDefault = -1, PlacementOverlay = -2, PlacementNewPane = -3, PlacementVolume = -4;

    public const uint InputPointerDown = 1, InputPointerMove = 2, InputPointerUp = 3, InputPointerLeave = 4,
        InputWheel = 5, InputPinch = 6, InputKey = 7, InputDoubleClick = 8;
    public const uint ButtonLeft = 0, ButtonMiddle = 1, ButtonRight = 2;
    public const uint DeltaPixels = 0, DeltaLines = 1;

    public const uint EventCrosshairMove = 1, EventClick = 2, EventVisibleRangeChanged = 3,
        EventNeedMoreHistory = 4, EventDrawingCreated = 5, EventDrawingModified = 6,
        EventDrawingRemoved = 7, EventDrawingSelected = 8, EventWarning = 9;
    public const uint TargetNone = 0, TargetSeries = 1, TargetMarker = 2, TargetDrawing = 3;
}

[StructLayout(LayoutKind.Sequential)]
public struct FerroChartOptions
{
    public uint Size;
    public uint Theme;
    public float Width;
    public float Height;
    public float Dpr;
}

[StructLayout(LayoutKind.Sequential)]
public struct FerroChartSeriesOptions
{
    public uint Size;
    public uint Color;
    public uint DownColor;
    public uint TopFill;
    public uint BottomFill;
    public float LineWidth;
    public uint LineStyle;
    public byte Step;
    public byte LastValueVisible;
    public byte PriceLineVisible;
    public byte Reserved;
    public double BaseValue;
    public int Shift;
    public uint PricePrecision;
}

[StructLayout(LayoutKind.Sequential)]
public struct FerroChartInput
{
    public uint Size;
    public uint Type;
    public ulong PointerId;
    public float X;
    public float Y;
    public uint Button;
    public uint Mods;
    public double TimeMs;
    public float DeltaX;
    public float DeltaY;
    public uint DeltaMode;
    public uint Phase;
    public float Scale;
    public uint Key;
    public uint KeyChar;
    public byte Pressed;
    public byte Reserved0, Reserved1, Reserved2;
}

[StructLayout(LayoutKind.Sequential)]
public struct FerroChartInputResult
{
    public byte Consumed;
    public byte Reserved0, Reserved1, Reserved2;
    public uint Cursor;
}

[StructLayout(LayoutKind.Sequential)]
public struct FerroChartEvent
{
    public uint Size;
    public uint Type;
    public ulong Pane;
    public float X;
    public float Y;
    public double LogicalIndex;
    public long Time;
    public double Price;
    public double Open;
    public double High;
    public double Low;
    public double Close;
    public double Volume;
    public double From;
    public double To;
    public long BarsBeforeStart;
    public ulong Drawing;
    public ulong TargetId;
    public ulong TargetIndex;
    public ulong Count;
    public uint TargetType;
    public uint Warning;
    public byte HasTime;
    public byte HasPoint;
    public byte HasBar;
    public byte HasDrawing;
    public uint Reserved;
}

public static class Api
{
    private const string Lib = "ferrochart";

    static Api()
    {
        NativeLibrary.SetDllImportResolver(typeof(Api).Assembly, Resolve);
    }

    /// Loads the library from $FERROCHART_LIB if set, else from a cargo
    /// target directory above the program (release first), else the
    /// default search path.
    private static IntPtr Resolve(string name, Assembly assembly, DllImportSearchPath? path)
    {
        if (name != Lib) return IntPtr.Zero;
        var env = Environment.GetEnvironmentVariable("FERROCHART_LIB");
        if (!string.IsNullOrEmpty(env)) return NativeLibrary.Load(env);
        var file = OperatingSystem.IsWindows() ? "ferrochart.dll"
            : OperatingSystem.IsMacOS() ? "libferrochart.dylib" : "libferrochart.so";
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir != null; dir = dir.Parent)
        {
            foreach (var profile in new[] { "release", "debug" })
            {
                var candidate = Path.Combine(dir.FullName, "target", profile, file);
                if (File.Exists(candidate)) return NativeLibrary.Load(candidate);
            }
        }
        return NativeLibrary.TryLoad(name, assembly, path, out var handle) ? handle : IntPtr.Zero;
    }

    [DllImport(Lib)] public static extern IntPtr ferrochart_last_error();
    [DllImport(Lib)] public static extern IntPtr ferrochart_version();
    [DllImport(Lib)] public static extern int ferrochart_options_default(ref FerroChartOptions o);
    [DllImport(Lib)] public static extern int ferrochart_create(ref FerroChartOptions o, out IntPtr chart);
    [DllImport(Lib)] public static extern void ferrochart_destroy(IntPtr chart);
    [DllImport(Lib)] public static extern int ferrochart_resize(IntPtr chart, float width, float height, float dpr);

    [DllImport(Lib)] public static extern int ferrochart_main_pane(IntPtr chart, out ulong pane);
    [DllImport(Lib)] public static extern int ferrochart_add_pane(IntPtr chart, float stretch, out ulong pane);
    [DllImport(Lib)] public static extern int ferrochart_remove_pane(IntPtr chart, ulong pane);

    [DllImport(Lib)] public static extern int ferrochart_series_options_default(ref FerroChartSeriesOptions o);
    [DllImport(Lib)]
    public static extern int ferrochart_add_series(IntPtr chart, ulong pane,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string kind, ref FerroChartSeriesOptions options, out ulong id);
    [DllImport(Lib, EntryPoint = "ferrochart_add_series")]
    public static extern int ferrochart_add_series_default(IntPtr chart, ulong pane,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string kind, IntPtr options, out ulong id);
    [DllImport(Lib)] public static extern int ferrochart_add_volume_series(IntPtr chart, ulong pane, ulong source, out ulong id);
    [DllImport(Lib)] public static extern int ferrochart_remove_series(IntPtr chart, ulong series);
    [DllImport(Lib)] public static extern int ferrochart_series_options(IntPtr chart, ulong series, ref FerroChartSeriesOptions o);
    [DllImport(Lib)] public static extern int ferrochart_apply_series_options(IntPtr chart, ulong series, ref FerroChartSeriesOptions o);

    [DllImport(Lib)]
    public static extern int ferrochart_set_data_ohlc(IntPtr chart, ulong series, long[] time, double[] open,
        double[] high, double[] low, double[] close, double[]? volume, nuint len);
    [DllImport(Lib)]
    public static extern int ferrochart_append_ohlc(IntPtr chart, ulong series, long[] time, double[] open,
        double[] high, double[] low, double[] close, double[]? volume, nuint len);
    [DllImport(Lib)]
    public static extern int ferrochart_set_data_value(IntPtr chart, ulong series, long[] time, double[] value,
        uint[]? color, nuint len);
    [DllImport(Lib)]
    public static extern int ferrochart_update_last(IntPtr chart, ulong series, long time, double open,
        double high, double low, double close, double volume);

    [DllImport(Lib)] public static extern IntPtr ferrochart_indicator_names();
    [DllImport(Lib)]
    public static extern int ferrochart_add_indicator(IntPtr chart, ulong source,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string name, [MarshalAs(UnmanagedType.LPUTF8Str)] string? paramsJson,
        long placement, out ulong id);
    [DllImport(Lib)] public static extern int ferrochart_remove_indicator(IntPtr chart, ulong indicator);

    [DllImport(Lib)] public static extern int ferrochart_fit_content(IntPtr chart);
    [DllImport(Lib)] public static extern int ferrochart_scroll_to_realtime(IntPtr chart);
    [DllImport(Lib)] public static extern int ferrochart_set_visible_logical_range(IntPtr chart, double from, double to);
    [DllImport(Lib)] public static extern int ferrochart_visible_logical_range(IntPtr chart, out double from, out double to);
    [DllImport(Lib)] public static extern int ferrochart_set_bar_spacing(IntPtr chart, double spacing);
    [DllImport(Lib)] public static extern int ferrochart_set_crosshair_mode(IntPtr chart, [MarshalAs(UnmanagedType.LPUTF8Str)] string mode);
    [DllImport(Lib)] public static extern int ferrochart_set_price_scale_mode(IntPtr chart, ulong pane, [MarshalAs(UnmanagedType.LPUTF8Str)] string mode);

    [DllImport(Lib)] public static extern void ferrochart_free_string(IntPtr s);
    [DllImport(Lib)] public static extern IntPtr ferrochart_save_state(IntPtr chart);
    [DllImport(Lib)] public static extern int ferrochart_load_state(IntPtr chart, [MarshalAs(UnmanagedType.LPUTF8Str)] string json);
    [DllImport(Lib)] public static extern int ferrochart_add_drawing(IntPtr chart, ulong pane, [MarshalAs(UnmanagedType.LPUTF8Str)] string json, out ulong id);
    [DllImport(Lib)] public static extern IntPtr ferrochart_drawing(IntPtr chart, ulong drawing);
    [DllImport(Lib)] public static extern int ferrochart_remove_drawing(IntPtr chart, ulong drawing);
    [DllImport(Lib)] public static extern int ferrochart_start_tool(IntPtr chart, [MarshalAs(UnmanagedType.LPUTF8Str)] string kind);
    [DllImport(Lib)] public static extern int ferrochart_cancel_tool(IntPtr chart);
    [DllImport(Lib)] public static extern int ferrochart_undo(IntPtr chart);
    [DllImport(Lib)] public static extern int ferrochart_redo(IntPtr chart);

    [DllImport(Lib)] public static extern int ferrochart_handle_input(IntPtr chart, ref FerroChartInput e, out FerroChartInputResult result);
    [DllImport(Lib)] public static extern int ferrochart_poll_event(IntPtr chart, ref FerroChartEvent e);
    [DllImport(Lib)] public static extern int ferrochart_pixel_size(IntPtr chart, out uint width, out uint height);
    [DllImport(Lib)] public static extern int ferrochart_render_rgba(IntPtr chart, double nowMs, byte[] buffer, nuint stride, nuint len);
    [DllImport(Lib)] public static extern int ferrochart_needs_redraw(IntPtr chart);
}
