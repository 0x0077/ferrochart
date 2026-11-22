// A thin C# wrapper over the C ABI: failures become exceptions with the
// library's message, strings returned by the library are copied and freed.

using System.Runtime.InteropServices;
using FerroChart.Native;

namespace FerroChart;

public sealed class FerroChartException(int status, string message) : Exception(message)
{
    public int Status { get; } = status;
}

public sealed class Chart : IDisposable
{
    private IntPtr _handle;

    public Chart(float width, float height, float dpr = 1, bool dark = false)
    {
        var o = new FerroChartOptions { Size = (uint)Marshal.SizeOf<FerroChartOptions>() };
        Check(Api.ferrochart_options_default(ref o));
        o.Theme = dark ? Const.ThemeDark : Const.ThemeLight;
        o.Width = width;
        o.Height = height;
        o.Dpr = dpr;
        Check(Api.ferrochart_create(ref o, out _handle));
    }

    public static string Version => Marshal.PtrToStringUTF8(Api.ferrochart_version()) ?? "";

    public static string[] IndicatorNames =>
        (Marshal.PtrToStringUTF8(Api.ferrochart_indicator_names()) ?? "").Split(',');

    internal IntPtr Handle => _handle != IntPtr.Zero ? _handle : throw new ObjectDisposedException(nameof(Chart));

    internal static int Check(int status)
    {
        if (status < 0)
            throw new FerroChartException(status, Marshal.PtrToStringUTF8(Api.ferrochart_last_error()) ?? "");
        return status;
    }

    /// Copies a string the library allocated and frees it.
    private static string TakeString(IntPtr s)
    {
        if (s == IntPtr.Zero)
            throw new FerroChartException(Status.ErrInvalidArgument,
                Marshal.PtrToStringUTF8(Api.ferrochart_last_error()) ?? "");
        try { return Marshal.PtrToStringUTF8(s) ?? ""; }
        finally { Api.ferrochart_free_string(s); }
    }

    public ulong AddPane(float stretch = 1)
    {
        Check(Api.ferrochart_add_pane(Handle, stretch, out var id));
        return id;
    }

    public ulong AddSeries(string kind, ulong pane = Const.MainPane)
    {
        Check(Api.ferrochart_add_series_default(Handle, pane, kind, IntPtr.Zero, out var id));
        return id;
    }

    public ulong AddSeries(string kind, FerroChartSeriesOptions options, ulong pane = Const.MainPane)
    {
        options.Size = (uint)Marshal.SizeOf<FerroChartSeriesOptions>();
        Check(Api.ferrochart_add_series(Handle, pane, kind, ref options, out var id));
        return id;
    }

    public static FerroChartSeriesOptions DefaultSeriesOptions()
    {
        var o = new FerroChartSeriesOptions { Size = (uint)Marshal.SizeOf<FerroChartSeriesOptions>() };
        Check(Api.ferrochart_series_options_default(ref o));
        return o;
    }

    public ulong AddVolumeSeries(ulong source, ulong pane = Const.MainPane)
    {
        Check(Api.ferrochart_add_volume_series(Handle, pane, source, out var id));
        return id;
    }

    public void SetData(ulong series, long[] time, double[] open, double[] high, double[] low, double[] close,
        double[]? volume = null)
    {
        var n = time.Length;
        if (open.Length != n || high.Length != n || low.Length != n || close.Length != n ||
            (volume != null && volume.Length != n))
            throw new ArgumentException("all columns must have the same length");
        Check(Api.ferrochart_set_data_ohlc(Handle, series, time, open, high, low, close, volume, (nuint)n));
    }

    public void SetData(ulong series, long[] time, double[] value)
    {
        if (value.Length != time.Length) throw new ArgumentException("all columns must have the same length");
        Check(Api.ferrochart_set_data_value(Handle, series, time, value, null, (nuint)time.Length));
    }

    public void UpdateLast(ulong series, long time, double open, double high, double low, double close,
        double volume = double.NaN) =>
        Check(Api.ferrochart_update_last(Handle, series, time, open, high, low, close, volume));

    public ulong AddIndicator(ulong source, string name, string? paramsJson = null,
        long placement = Const.PlacementDefault)
    {
        Check(Api.ferrochart_add_indicator(Handle, source, name, paramsJson, placement, out var id));
        return id;
    }

    public void FitContent() => Check(Api.ferrochart_fit_content(Handle));
    public void SetVisibleLogicalRange(double from, double to) =>
        Check(Api.ferrochart_set_visible_logical_range(Handle, from, to));

    public (double From, double To) VisibleLogicalRange
    {
        get
        {
            Check(Api.ferrochart_visible_logical_range(Handle, out var from, out var to));
            return (from, to);
        }
    }

    public void SetCrosshairMode(string mode) => Check(Api.ferrochart_set_crosshair_mode(Handle, mode));

    public ulong AddDrawing(string json, ulong pane = Const.MainPane)
    {
        Check(Api.ferrochart_add_drawing(Handle, pane, json, out var id));
        return id;
    }

    public string Drawing(ulong id) => TakeString(Api.ferrochart_drawing(Handle, id));
    public void StartTool(string kind) => Check(Api.ferrochart_start_tool(Handle, kind));
    public bool Undo() => Check(Api.ferrochart_undo(Handle)) == 1;
    public bool Redo() => Check(Api.ferrochart_redo(Handle)) == 1;

    public string SaveState() => TakeString(Api.ferrochart_save_state(Handle));
    public void LoadState(string json) => Check(Api.ferrochart_load_state(Handle, json));

    /// Passes an input event; returns whether the chart used it.
    public bool HandleInput(FerroChartInput e)
    {
        e.Size = (uint)Marshal.SizeOf<FerroChartInput>();
        Check(Api.ferrochart_handle_input(Handle, ref e, out var result));
        return result.Consumed != 0;
    }

    /// Takes all pending events.
    public List<FerroChartEvent> PollEvents()
    {
        var events = new List<FerroChartEvent>();
        while (true)
        {
            var e = new FerroChartEvent { Size = (uint)Marshal.SizeOf<FerroChartEvent>() };
            if (Check(Api.ferrochart_poll_event(Handle, ref e)) == 0) return events;
            events.Add(e);
        }
    }

    public bool NeedsRedraw => Check(Api.ferrochart_needs_redraw(Handle)) == 1;

    public (int Width, int Height) PixelSize
    {
        get
        {
            Check(Api.ferrochart_pixel_size(Handle, out var w, out var h));
            return ((int)w, (int)h);
        }
    }

    /// Draws the chart (CPU renderer) into `pixels` (RGBA8, `width * 4` bytes
    /// per row), allocating it when it is null or of the wrong size.
    public byte[] RenderRgba(double nowMs, byte[]? pixels = null)
    {
        var (w, h) = PixelSize;
        var len = w * h * 4;
        if (pixels == null || pixels.Length != len) pixels = new byte[len];
        Check(Api.ferrochart_render_rgba(Handle, nowMs, pixels, (nuint)(w * 4), (nuint)len));
        return pixels;
    }

    public void Dispose()
    {
        Api.ferrochart_destroy(_handle);
        _handle = IntPtr.Zero;
    }
}
