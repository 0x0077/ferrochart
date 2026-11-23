<!-- source: a58b5627d3 -->
# クックブック

FerroChartを使うアプリケーション向けに、目的別の実装例をまとめました。ブラウザ向けの各レシピは、`@ferrochart/core`をインストールしたブラウザ用バンドラーで使う独立したTypeScriptモジュールです。ReactのレシピではReactも必要です。`host`には、たとえば`document.body`のように、DOMに追加済みで幅が0ではない`HTMLElement`を渡してください。ビューを取り除くときは、返されたクリーンアップ関数を呼びます。価格アラートのレシピでは、代わりに`update`と`destroy`メソッドを返します。[Webの導入](web.md)も参照してください。

サンプルには合成データを使っています。タイムスタンプはUTCミリ秒で、足は時刻の昇順に並べます。ネットワークを使うレシピでは、独自のフィードが満たすべき入力形式を示しています。市場データサービスは付属しません。Pythonのレシピでは必要なパッケージを示し、PNGをカレントディレクトリに書き出します。

## WebSocketでローソク足をリアルタイム更新する

テキストメッセージごとに1本のOHLCバーをJSONで送る、独自のWebSocketエンドポイントにチャートを接続します。

```ts
import { FerroChart, type Bar } from '@ferrochart/core';

export async function mountLiveChart(host: HTMLElement, feedUrl: string) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const candles = chart.addCandlestickSeries();
  const socket = new WebSocket(feedUrl);
  const receive = (event: MessageEvent<string>) => {
    try {
      const bar = JSON.parse(event.data) as Bar;
      candles.update(bar);
    } catch (error) {
      console.error('Invalid or out-of-order bar', error);
    }
  };
  socket.addEventListener('message', receive);
  socket.addEventListener('error', () => console.error('WebSocket error'));
  return () => {
    socket.removeEventListener('message', receive);
    socket.close();
    chart.destroy();
    canvas.remove();
  };
}
```

- フィードはUTCミリ秒の`time`と`open`、`high`、`low`、`close`（任意で`volume`）を送る必要があります。データ提供元の形式を変換してから`update`を呼んでください。
- 開始時刻が同じなら最後の足を置き換え、後の時刻なら新しい足を追加します。前の時刻では例外になります。各更新には約定の差分ではなく、その時点の足全体を渡します。
- アンマウント時には返されたクリーンアップ関数を呼びます。再接続はフィードアダプター側で処理してください。[リアルタイム更新](data.md#real-time-updates)を参照してください。

## 左へスクロールしたときに過去データを読み込む

表示範囲を動かさずに前のページを読み込みます。この独立したサンプルでは、バックエンドの代わりにメモリ上の過去データを使います。

```ts
import { FerroChart } from '@ferrochart/core';

export async function mountHistory(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const archive = Array.from({ length: 120 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const initial = archive.slice(-40);
  const candles = chart.addCandlestickSeries();
  let oldest = initial[0].time;
  let loading = false, exhausted = false, disposed = false;
  const button = document.createElement('button');
  button.textContent = 'Load earlier / retry';
  host.append(button);
  async function loadBefore(before: number) {
    return archive.filter(bar => bar.time < before).slice(-40);
  }
  async function loadOlder() {
    if (loading || exhausted || disposed) return;
    loading = true;
    button.disabled = true;
    try {
      const older = await loadBefore(oldest);
      if (disposed) return;
      if (!older.length) { exhausted = true; return; }
      candles.prepend(older);
      oldest = older[0].time;
    } catch (error) {
      console.error('History load failed; use the retry button', error);
    } finally {
      loading = false;
      button.disabled = exhausted;
    }
  }
  chart.on('needMoreHistory', loadOlder);
  button.onclick = () => { void loadOlder(); };
  candles.setData(initial);
  chart.fitContent();
  return () => {
    disposed = true;
    chart.destroy();
    canvas.remove();
    button.remove();
  };
}
```

- `loadBefore`をバックエンドへのリクエストに置き換えてください。`before`より前の、重複のないタイムスタンプを昇順で返します。出来高カラムの有無は初期データとそろえてください。
- `needMoreHistory`は先頭の足ごとに一度発火します。空の応答でページングを終了し、リクエストに失敗した場合はボタンで明示的に再試行できます。
- `prepend`は既存の論理インデックスと表示範囲を維持します。ページを追加するたびに`fitContent`を呼ばないでください。[過去データの読み込み](data.md#loading-older-history)を参照してください。

## インジケーターを独立したペインに表示する

同じローソク足シリーズに、重ねて表示する移動平均とRSI用のペインを追加します。

```ts
import { FerroChart } from '@ferrochart/core';

export async function mountIndicators(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const candles = chart.addCandlestickSeries();
  candles.setData(bars);
  chart.fitContent();
  chart.addIndicator(candles, 'ema', { n: 20 }, 'overlay');
  chart.addIndicator(candles, 'rsi', { n: 14 }, 'newPane');
  return () => { chart.destroy(); canvas.remove(); };
}
```

- 配置は第4引数で指定します。`'overlay'`、`'newPane'`、既存のペインIDなどを使えます。デフォルトはインジケーターによって異なります。
- インジケーターは元のシリーズのデータに追従するため、別途データを渡す必要はありません。選んだ期間に必要な過去データを用意してください。[インジケーターの配置](indicators.md#placement)を参照してください。

## マーカーに売買記録を関連付ける

ユーザーがマーカーをクリックしたときに、アプリケーション側の取引オブジェクトを取り出します。

```ts
import { FerroChart, type Marker } from '@ferrochart/core';

type Trade = { id: number; time: number; side: 'buy' | 'sell'; quantity: number };

export async function mountTradeMarkers(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const candles = chart.addCandlestickSeries();
  candles.setData(bars);
  chart.fitContent();
  const trades: Trade[] = [
    { id: 1, time: bars[10].time, side: 'buy', quantity: 2 },
    { id: 2, time: bars[30].time, side: 'sell', quantity: 1 },
  ];
  const markers: (Marker & { trade: Trade })[] = trades.map(trade => ({
    id: trade.id, time: trade.time, trade,
    position: trade.side === 'buy' ? 'belowBar' : 'aboveBar',
    shape: trade.side === 'buy' ? 'arrowUp' : 'arrowDown',
    text: trade.side,
  }));
  candles.setMarkers(markers);
  const info = document.createElement('p');
  info.textContent = 'Click a buy or sell marker';
  host.append(info);
  chart.on('click', ({ target }) => {
    if (target?.type !== 'marker' || target.series.id !== candles.id) return;
    const marker = markers.find(candidate => candidate === target.marker);
    if (marker) info.textContent = `Trade ${target.id}: ${marker.trade.side} ${marker.trade.quantity}`;
  });
  return () => { chart.destroy(); canvas.remove(); info.remove(); };
}
```

- `id`にはアプリケーション側の識別子として、安全に表現できる非負整数を指定します。`target.marker`は`setMarkers`に渡したオブジェクトそのもので、独自の`trade`フィールドも保持されます。
- イベントのインデックスは、元の配列順ではなく時刻順に並べたマーカーを指します。折り畳まれたバッジの型は`'markerOverflow'`です。必要なら別途処理してください。
- `setMarkers`はマーカーの一覧を置き換えます。[マーカー](markers.md#markers)と[クリックイベント](crosshair-events.md#click)を参照してください。

## 価格アラートのラインを描画して上抜けを検出する

しきい値を表示し、受信した足の終値が上抜けたときにアプリケーションへ通知します。

```ts
import { FerroChart, type Bar } from '@ferrochart/core';

export async function mountPriceAlert(host: HTMLElement, notify: (price: number) => void) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const candles = chart.addCandlestickSeries();
  const first = { time: Date.UTC(2025, 0, 1), open: 100, high: 102, low: 99, close: 101 };
  candles.setData([first]);
  const threshold = 105;
  candles.createPriceLine({ price: threshold, title: 'Alert', color: '#ef5350', lineStyle: 'dashed' });
  let previous = first.close;
  return {
    update(bar: Bar) {
      candles.update(bar);
      const crossed = previous < threshold && bar.close >= threshold;
      previous = bar.close;
      if (crossed) notify(bar.close);
    },
    destroy() { chart.destroy(); canvas.remove(); },
  };
}
```

- フィードの足は、返された`update`メソッドに渡してください。ライン自体が価格を監視したり、アラートを通知したりするわけではありません。
- これは終値同士を比較して上抜けを検出し、形成中の足の更新も対象にします。再び通知されるのは、価格が一度しきい値を下回ってから上抜けたときです。バックグラウンドサービスではありません。
- アンマウント時には`destroy`を呼んでください。[価格ライン](markers.md#price-lines)と[リアルタイム更新](data.md#real-time-updates)を参照してください。

## 2つのチャートを同期する

2つのチャートでクロスヘアの時刻と表示時間範囲を共有します。

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

export async function mountSyncedCharts(host: HTMLElement) {
  const canvases = [document.createElement('canvas'), document.createElement('canvas')];
  for (const canvas of canvases) {
    canvas.style.cssText = 'display:block;width:100%;height:260px';
    host.append(canvas);
  }
  const first = await FerroChart.create(canvases[0]);
  let second: FerroChart;
  try { second = await FerroChart.create(canvases[1]); }
  catch (error) { first.destroy(); canvases.forEach(canvas => canvas.remove()); throw error; }
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  first.addCandlestickSeries().setData(bars);
  second.addSeries('line').setData(bars.map(bar => ({ time: bar.time, value: bar.close })));
  first.fitContent();
  second.fitContent();
  const group = new SyncGroup();
  first.joinSync(group, { crosshair: true, visibleRange: true });
  second.joinSync(group, { crosshair: true, visibleRange: true });
  return () => {
    first.destroy(); second.destroy();
    group.free();
    canvases.forEach(canvas => canvas.remove());
  };
}
```

- 同期にはバーのインデックスではなく時刻を使います。それぞれのチャートにデータを用意する必要があります。バー間隔の制限によって、表示できる範囲が制約される場合があります。
- チャートを破棄するとグループから離脱します。グループは所属するチャートの後に解放してください。描画の同期は別の設定で、デフォルトでは無効です。[チャートの同期](sync.md#time-not-bar-numbers)を参照してください。

## 描画とビューポートを保存する

チャートのデータはアプリケーション側で再構築し、ユーザーの表示範囲と描画はlocalStorageに保存します。

```ts
import { FerroChart, type ChartState } from '@ferrochart/core';

export async function mountSavedChart(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const chart = await FerroChart.create(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const candles = chart.addCandlestickSeries();
  candles.setData(bars);
  chart.fitContent();
  const key = 'ferrochart-cookbook-view';
  const status = document.createElement('p');
  const draw = document.createElement('button');
  draw.textContent = 'Draw trend line';
  draw.onclick = () => chart.drawings.startTool('trend_line');
  const save = document.createElement('button');
  save.textContent = 'Save view and drawings';
  save.onclick = () => {
    try {
      localStorage.setItem(key, JSON.stringify(chart.saveState()));
      status.textContent = 'Saved';
    } catch (error) { status.textContent = String(error); }
  };
  host.append(draw, save, status);
  try {
    const stored = localStorage.getItem(key);
    if (stored) chart.loadState(JSON.parse(stored) as ChartState);
  } catch (error) { status.textContent = `Could not restore: ${String(error)}`; }
  return () => { chart.destroy(); canvas.remove(); draw.remove(); save.remove(); status.remove(); };
}
```

- `loadState`の前に、データ、シリーズ、インジケーターと同じペイン構成を再作成してください。状態には市場データやシリーズ定義は含まれません。
- 復元すると描画が置き換わり、取り消し履歴が消去されます。互換性のないレイアウトや銘柄には別の保存キーを使い、保存や復元のエラーを表示してください。
- アンマウントする前にSaveボタンを押してください。[状態に含まれるもの](state.md#what-a-state-contains)と[状態の読み込み](state.md#loading-a-state)を参照してください。

## Reactでチャートを作成・破棄する

初期化の完了前にアンマウントされた場合も含め、GPUリソースの寿命をcanvasのマウント期間に合わせます。

```tsx
'use client';

import { useEffect, useRef } from 'react';
import { FerroChart, type OhlcRow } from '@ferrochart/core';

const bars: OhlcRow[] = Array.from({ length: 60 }, (_, i) => ({
  time: Date.UTC(2025, 0, 1) + i * 60_000,
  open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
}));

export function PriceChart() {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    let disposed = false;
    let chart: FerroChart | undefined;
    void FerroChart.create(element).then(created => {
      if (disposed) { created.destroy(); return; }
      chart = created;
      created.addCandlestickSeries().setData(bars);
      created.fitContent();
    }).catch(error => console.error('Chart initialization failed', error));
    return () => { disposed = true; chart?.destroy(); };
  }, []);
  return <canvas ref={canvas} style={{ display: 'block', width: '100%', height: 360 }} />;
}
```

- クライアントコンポーネントで`<PriceChart />`をレンダリングしてください。effectはブラウザ内で実行されます。破棄済みかどうかのチェックにより、Reactの開発時に行われるセットアップとクリーンアップのサイクルにも対応します。
- リアルタイムデータではシリーズをrefに保持して`update`を呼び、ティックのたびにチャートを作り直さないでください。[React](frameworks.md#react)を参照してください。

## ライトテーマとダークテーマを切り替える

ビューポートと描画を引き継いで、別のテーマのチャートを作り直します。

```ts
import { FerroChart } from '@ferrochart/core';

export async function mountThemeSwitch(host: HTMLElement) {
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  host.append(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  let theme: 'light' | 'dark' = 'light';
  const create = async () => {
    const result = await FerroChart.create(canvas, { theme });
    result.addCandlestickSeries().setData(bars);
    return result;
  };
  let chart: FerroChart | undefined = await create();
  chart.fitContent();
  let disposed = false;
  const button = document.createElement('button');
  button.textContent = 'Switch theme';
  host.append(button);
  button.onclick = async () => {
    if (!chart) return;
    button.disabled = true;
    const state = chart.saveState();
    chart.destroy();
    chart = undefined;
    theme = theme === 'light' ? 'dark' : 'light';
    try {
      const next = await create();
      if (disposed) { next.destroy(); return; }
      chart = next;
      chart.loadState(state);
      button.disabled = false;
    } catch (error) { console.error('Theme rebuild failed', error); }
  };
  return () => {
    disposed = true;
    chart?.destroy();
    canvas.remove(); button.remove();
  };
}
```

- テーマは作成時のオプションであり、`applyOptions`では変更できません。状態を復元する前に同じデータ、シリーズ、ペインを再作成し、イベントの購読も登録し直してください。
- 色を明示していないシリーズは新しいテーマに従います。このサンプルでは再作成中の切り替えを無効にし、失敗した場合は自動再試行せずエラーを報告します。[テーマ](styling.md#theme)と[状態に含まれないもの](state.md#what-it-does-not-contain)を参照してください。

## クロスヘア用のDOMツールチップを作る

ポインターの下にある足の終値を、独自のHTMLオーバーレイに表示します。

```ts
import { FerroChart } from '@ferrochart/core';

export async function mountTooltip(host: HTMLElement) {
  const wrapper = document.createElement('div');
  wrapper.style.position = 'relative';
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'display:block;width:100%;height:360px';
  const tip = document.createElement('div');
  tip.hidden = true;
  tip.style.cssText = 'position:absolute;pointer-events:none;background:#111;color:#fff;padding:6px';
  wrapper.append(canvas, tip);
  host.append(wrapper);
  const chart = await FerroChart.create(canvas);
  const bars = Array.from({ length: 60 }, (_, i) => ({
    time: Date.UTC(2025, 0, 1) + i * 60_000,
    open: 100 + i, high: 102 + i, low: 99 + i, close: 101 + i,
  }));
  const candles = chart.addCandlestickSeries();
  candles.setData(bars);
  chart.fitContent();
  chart.on('crosshairMove', ({ point }) => {
    if (!point?.bar || point.time === null || point.x === undefined || point.y === undefined) {
      tip.hidden = true;
      return;
    }
    tip.textContent = `${new Date(point.time).toISOString()} · ${point.bar.close.toFixed(2)}`;
    tip.style.left = `${point.x + 12}px`;
    tip.style.top = `${point.y + 12}px`;
    tip.hidden = false;
  });
  return () => { chart.destroy(); wrapper.remove(); };
}
```

- 座標はcanvasを基準とするCSSピクセルです。オーバーレイを同じ位置指定用ラッパーに配置してください。フィード由来のテキストには`textContent`を使います。
- プロットから離れると`point: null`が送られ、欠損箇所には足がない場合もあります。同期されたクロスヘアにはポインター座標がないため、このサンプルでは非表示にします。[クロスヘアイベント](crosshair-events.md#crosshairmove)を参照してください。

## pandasのDataFrameをPNGに出力する

ウィンドウを開かずに、pandasのDataFrameにあるOHLCVデータを描画して画像として保存します。

```python
import pandas as pd
import ferrochart as fc

df = pd.DataFrame({
    "time": pd.to_datetime([
        "2026-01-01T00:00:00Z", "2026-01-01T00:01:00Z", "2026-01-01T00:02:00Z"
    ]),
    "open": [100.0, 101.0, 100.5],
    "high": [102.0, 102.5, 103.0],
    "low": [99.0, 100.0, 100.0],
    "close": [101.0, 100.5, 102.0],
    "volume": [1200.0, 950.0, 1400.0],
})
chart = fc.Chart(theme="dark", width=800, height=450)
candles = chart.add_series("candlestick")
candles.set_data(df)
chart.add_volume_series(candles)
chart.fit_content()
chart.to_png("pandas-chart.png")
```

- `ferrochart`と`pandas`をインストールしてください。日時カラムはUTCミリ秒に変換されます。このサンプルでは明示的なUTCタイムスタンプを使っています。
- 行を時刻順に並べ、すべてのOHLCカラムを用意してください。`to_png(path)`はPNGを書き出します。`show()`は不要です。[Pythonのクイックスタート](python.md#quick-start)を参照してください。

## CPUのみのサーバーでPNGを描画する

ウィンドウやGPUアダプターを使わず、ファイルや独自のHTTPレスポンスに使えるPNGバイト列を生成します。

```python
from pathlib import Path
import ferrochart as fc

chart = fc.Chart(width=800, height=450)
chart.add_series("line", color="#2196f3").set_data({
    "time": [1767225600000, 1767225660000, 1767225720000],
    "value": [100.0, 101.5, 100.8],
})
chart.fit_content()
png = chart.to_png(backend="cpu")
Path("server-chart.png").write_bytes(png)
```

- `ferrochart`をインストールしてください。`backend="cpu"`は`Chart`のコンストラクターではなく`to_png`の引数です。この経路ではGPUアダプターを作成しません。
- パスを渡さなければ、`to_png`はバイト列を返します。ブラウザのcanvasに頼らず、出力に必要なサイズを指定してください。[CPUによるオフスクリーン描画](offscreen.md#on-the-cpu)と[Python](python.md#quick-start)を参照してください。
