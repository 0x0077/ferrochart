// FerroChart from C# through the C ABI: candlesticks with volume,
// indicators, a drawing made with the trend line tool, simulated input and
// a live feed, rendered offscreen to a PNG. Exits with 1 if anything the
// example checks goes wrong, so scripts/test-ffi-csharp.sh can run it as a
// test.
//
//   cargo build -p ferrochart-ffi --release
//   dotnet run --project examples/csharp -- [output directory]
//
// The library is found through $FERROCHART_LIB or the cargo target
// directory (see Native.cs).

using System.Diagnostics;
using FerroChart;
using FerroChart.Example;
using FerroChart.Native;

var outDir = args.Length > 0 ? args[0] : ".";
Directory.CreateDirectory(outDir);

try
{
    Run(outDir);
    return 0;
}
catch (Exception e) when (e is FerroChartException or InvalidOperationException)
{
    Console.Error.WriteLine($"error: {e.Message}");
    return 1;
}

static void Expect(bool condition, string what)
{
    if (!condition) throw new InvalidOperationException($"expected {what}");
}

static void Run(string outDir)
{
    Console.WriteLine($"FerroChart {Chart.Version}, {Chart.IndicatorNames.Length} indicators");

    // A deterministic random walk of hourly bars (from 2023-11-14 21:00 UTC).
    const int n = 2000;
    const long hour = 3_600_000;
    var t = new long[n];
    var o = new double[n];
    var h = new double[n];
    var l = new double[n];
    var c = new double[n];
    var v = new double[n];
    var rng = new Random(7);
    var price = 100.0;
    for (var i = 0; i < n; i++)
    {
        var next = Math.Max(1, price * (1 + (rng.NextDouble() - 0.5) * 0.02));
        t[i] = 1_699_999_200_000 + i * hour;
        o[i] = price;
        c[i] = next;
        h[i] = Math.Max(price, next) * (1 + rng.NextDouble() * 0.005);
        l[i] = Math.Min(price, next) * (1 - rng.NextDouble() * 0.005);
        v[i] = 500 + rng.Next(1500);
        price = next;
    }

    using var chart = new Chart(960, 600, dpr: 2, dark: true);
    var candles = chart.AddSeries("candlestick");
    chart.SetData(candles, t, o, h, l, c, v);
    chart.AddVolumeSeries(candles);
    chart.AddIndicator(candles, "ema", """{"n": 20}""");
    chart.AddIndicator(candles, "boll");
    chart.AddIndicator(candles, "rsi", placement: Const.PlacementNewPane);
    chart.SetVisibleLogicalRange(n - 200, n + 5);

    // Errors carry the library's message.
    try
    {
        chart.AddIndicator(candles, "ema", """{"length": 20}""");
        Expect(false, "an error for an unknown parameter");
    }
    catch (FerroChartException e)
    {
        Console.WriteLine($"rejected as expected: {e.Message}");
        Expect(e.Status == Status.ErrInvalidArgument && e.Message.Contains("'length'"), "the bad key in the message");
    }

    // The first frame lays the chart out; input coordinates refer to it.
    var pixels = chart.RenderRgba(0);
    var (width, height) = chart.PixelSize;
    Expect(width == 1920 && height == 1200, "a 1920 × 1200 image at dpr 2");

    var clock = 1000.0;
    void Click(float x, float y)
    {
        chart.HandleInput(new FerroChartInput { Type = Const.InputPointerDown, X = x, Y = y, TimeMs = clock });
        chart.HandleInput(new FerroChartInput { Type = Const.InputPointerUp, X = x, Y = y, TimeMs = clock + 50 });
        clock += 1000; // far apart: no double clicks
    }

    // A plain click, then a trend line drawn with the tool.
    Click(400, 200);
    chart.StartTool("trend_line");
    Click(250, 260);
    Click(700, 120);
    // Zoom in a little with the wheel over the plot.
    chart.HandleInput(new FerroChartInput { Type = Const.InputWheel, X = 600, Y = 200, DeltaY = -120, TimeMs = clock });

    ulong? drawing = null;
    var clicks = 0;
    foreach (var e in chart.PollEvents())
    {
        switch (e.Type)
        {
            case Const.EventClick:
                clicks++;
                Console.WriteLine($"click at bar {e.LogicalIndex:F1}, " +
                    $"{DateTimeOffset.FromUnixTimeMilliseconds(e.Time):u}, price {e.Price:F2}");
                break;
            case Const.EventDrawingCreated:
                drawing = e.Drawing;
                break;
        }
    }
    Expect(clicks >= 1, "a click event");
    Expect(drawing.HasValue, "a drawing created by the tool");
    Console.WriteLine($"drawing {drawing}: {chart.Drawing(drawing!.Value)}");

    // A live feed: update the last bar, append new ones, draw when needed.
    var (barTime, barOpen, barHigh, barLow, barVolume) = (t[^1], o[^1], h[^1], l[^1], v[^1]);
    var frames = 0;
    var sw = Stopwatch.StartNew();
    for (var tick = 0; tick < 300; tick++)
    {
        price *= 1 + (rng.NextDouble() - 0.5) * 0.004;
        if (tick % 60 == 59) // a new bar every 60 ticks
            (barTime, barOpen, barHigh, barLow, barVolume) = (barTime + hour, price, price, price, 0);
        barHigh = Math.Max(barHigh, price);
        barLow = Math.Min(barLow, price);
        barVolume += 10;
        chart.UpdateLast(candles, barTime, barOpen, barHigh, barLow, price, barVolume);
        if (chart.NeedsRedraw)
        {
            pixels = chart.RenderRgba(clock + tick * 16, pixels);
            frames++;
        }
    }
    sw.Stop();
    Expect(frames > 0, "redraws during the live feed");
    Console.WriteLine($"live feed: 300 ticks, {frames} frames, {sw.Elapsed.TotalMilliseconds / frames:F2} ms per frame (CPU)");

    // State round trip into a fresh chart built the same way.
    var state = chart.SaveState();
    using (var copy = new Chart(960, 600, dpr: 2, dark: true))
    {
        var s = copy.AddSeries("candlestick");
        copy.SetData(s, t, o, h, l, c, v);
        copy.AddIndicator(s, "rsi", placement: Const.PlacementNewPane);
        copy.LoadState(state);
        Expect(copy.SaveState().Contains("trend_line"), "the drawing in the loaded state");
    }
    File.WriteAllText(Path.Combine(outDir, "ferrochart-csharp-state.json"), state);

    pixels = chart.RenderRgba(clock + 10_000, pixels);
    Expect(pixels[3] == 255, "an opaque image");
    var png = Path.Combine(outDir, "ferrochart-csharp.png");
    Png.Write(png, pixels, width, height);
    Console.WriteLine($"wrote {png}");
}
