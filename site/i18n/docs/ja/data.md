<!-- source: 8ace0a0ae7 -->
# データ

この章では、シリーズに足や値を渡す方法を扱います。データ形式、検証ルール、リアルタイム更新、過去データの読み込み、データの読み出しです。

## カラムと行

`setData`はシリーズのデータをすべて置き換えます。受け付ける形は2つあり、カラムのオブジェクト（フィールドごとに1つの配列）か、行オブジェクトの配列です。

### カラム

OHLCシリーズ（ローソク足、OHLCバー、平均足、変換シリーズ）は`OhlcData`を受け取ります。

| カラム | 型 | 備考 |
|---|---|---|
| `time` | `BigInt64Array`、`Float64Array`または`number[]` | 足の開始時刻。UTCミリ秒で、狭義単調増加 |
| `open`, `high`, `low`, `close` | `Float64Array`または`number[]` | |
| `volume` | `Float64Array`または`number[]` | 省略可 |

値シリーズ（ライン、エリア、ベースライン、ヒストグラム、ポイント）は`ValueData`を受け取ります。

| カラム | 型 | 備考 |
|---|---|---|
| `time` | `BigInt64Array`、`Float64Array`または`number[]` | UTCミリ秒で、狭義単調増加 |
| `value` | `Float64Array`または`number[]` | `NaN`は欠損 |
| `color` | `(string \| null)[]`または`Uint32Array` | 省略可。ポイントごとの色で、ヒストグラムでのみ描画されます（[シリーズの種類](series.md#histogram)を参照） |

すべてのカラムは同じ長さでなければなりません。

```ts
candles.setData({
  time: new BigInt64Array([1704067200000n, 1704067260000n, 1704067320000n]),
  open: new Float64Array([100.0, 100.4, 100.1]),
  high: new Float64Array([100.6, 100.5, 100.9]),
  low: new Float64Array([99.8, 99.9, 100.0]),
  close: new Float64Array([100.4, 100.1, 100.8]),
  volume: new Float64Array([1200, 900, 1500]),
});

line.setData({ time: [1704067200000, 1704067260000], value: [100.4, 100.1] });
```

### 行

オブジェクトの配列も受け付けます。OHLCシリーズでは`OhlcRow`（`{ time, open, high, low,
close, volume? }`）、値シリーズでは`ValueRow`（`{ time, value?, color? }`）です。行では`time`に`Date`も使えます。

```ts
candles.setData([
  { time: new Date('2024-01-01T00:00:00Z'), open: 100.0, high: 100.6, low: 99.8, close: 100.4 },
  { time: new Date('2024-01-01T00:01:00Z'), open: 100.4, high: 100.5, low: 99.9, close: 100.1 },
]);

line.setData([
  { time: 1704067200000, value: 100.4 },
  { time: 1704067260000 },                 // no value: a gap
  { time: 1704067320000, value: 100.8 },
]);
```

`value`のない行（または`NaN`の行）は欠損です。OHLCのフィールドが欠けていると、そのフィールド名を示すエラーが投げられます（`data[3] has no 'open'`）。`volume`を持つ行と持たない行が混在する場合、欠けている出来高は0になります。

### どちらを使うか

行は他の処理より先にカラムへ変換されるため、どちらでも結果は同じです。データがすでにオブジェクトとして届く場合（JSON API、数百本程度の足）で、そのほうがコードが簡単になるなら行を使ってください。大きなデータにはカラムを使います。型付き配列はカラムごとに1回の`memcpy`でエンジンにコピーされ、足ごとのオブジェクトを読む必要がないため、100万本の足の`setData`も数十ミリ秒で終わります。バイナリデータを受け取る場合（たとえばWebSocketやファイルから）は、型付き配列を直接構築し、オブジェクトを完全に省いてください。

### 時刻

時刻はUnixエポックからのUTCミリ秒で、整数です。

- `BigInt64Array`の時刻はそのまま使われます。
- `Float64Array`と`number[]`の時刻は整数に変換されます。有限でない値、小数部を持つ値、±9·10¹⁵ を超える値は例外になります（`time[5] is not an integer millisecond timestamp`）。
- 行と`update`では、`time`にnumber、bigint、`Date`のいずれも使えます。

ソースが秒単位の場合は1000を掛けてください。

チャートは時刻をUTCで表示します。

## 検証

`setData`と`update`は最初にデータを検査します。問題があれば例外を投げ、シリーズは以前のデータを保持します。

| ルール | エラーメッセージ |
|---|---|
| 時刻が狭義単調増加（重複なし） | `time is not strictly increasing at index 7` |
| すべてのカラムが`time`と同じ長さ | ``column `open` has a different length than `time` `` |
| 価格と出来高が有限（欠損の足を除く） | `invalid OHLC values in bar 12` |
| `high` ≥ max(`open`, `close`) かつ`low` ≤ min(`open`, `close`) | `invalid OHLC values in bar 12` |
| カラムが数値の配列または型付き配列 | `column 'high' must be a Float64Array or an array of numbers` |

メッセージ中のインデックスは、渡したデータ内での位置です。

4つの価格がすべて`NaN`の足は欠損の足です（[欠損](#gaps)を参照）。これは有効なデータで、出来高は0に設定されます。

## 欠損

- 値シリーズでは、`NaN`の値（または`value`のない行）が欠損です。ライン、エリア、ベースラインはそこで途切れ、ヒストグラムとポイントは何も描画しません。
- OHLCシリーズでは、始値・高値・安値・終値がすべて`NaN`の足が欠損で、何も描画されません。

欠損に対しては`dataAt`は`null`を返します。

### 1つの時間軸上の複数のシリーズ

チャートのすべてのシリーズは1つの時間軸を共有します。論理インデックス42はどのシリーズでも同じ時刻です。シリーズごとに時刻が異なる場合、軸はそれらの時刻の和集合になります。他のシリーズにしかない時刻には、そのシリーズの点はありません。OHLCシリーズ、ヒストグラム、ポイントはそこに何も表示せず、ライン、エリア、ベースラインは隣り合う点を結びます。新しい時刻が軸の既存の足と足の間に入る場合、軸は再構築され、チャートは`warning`イベントを送ります。

## 足の品質フラグ

ローソク足、OHLCバー、平均足のシリーズでは、`setBarFlags`でアプリケーションが補完またはクリップしたデータを示せます。フラグ自体は価格を変更しません。まず加工済みのOHLCデータを渡してください。

```ts
const start = Date.UTC(2024, 0, 1);
candles.setData([
  { time: start, open: 100, high: 102, low: 99, close: 101 },
  { time: start + 60_000, open: 101, high: 101, low: 101, close: 101 },
  { time: start + 120_000, open: 101, high: 104, low: 100, close: 103 },
]);
candles.applyOptions({ filledBarColor: '#78909c' });
candles.setBarFlags([
  { time: start + 60_000, filled: true },
  { time: start + 120_000, clipped: true },
]);
// Clear all quality flags when they are no longer needed:
// candles.setBarFlags([]);
```

- `filled`は、実際の取引を伴わずにデータ提供元が補完した足を示します。`filledBarColor`（デフォルトはシリーズのメインカラー、不透明度40%）で中空に描画されます。実際にデータがない足は[欠損](#gaps)として表すこともできます。このフラグが欠損データを補完するわけではありません。
- `clipped`は、外れ値となるヒゲをクリップした足を示します。ヒゲの両端に短い目盛りが表示されます。元の高値と安値は時刻をキーにアプリケーション側で保持し、ツールチップで表示してください。チャートが持つのは、渡されたクリップ後の値だけです。1本の足に両方のフラグを設定できます。
- 呼び出しごとにすべてのフラグが置き換わり、`[]`で消去されます。時刻はUTCミリ秒（データと同様に`Date`/`bigint`も可）で、重複する時刻では最後の指定が優先されます。ズームアウトで足が集約される場合、集約後の足が`filled`になるのはすべての足が`filled`の場合だけで、いずれかが`clipped`なら`clipped`になります。

Pythonでは、`times`と同じ長さの真偽値配列を使って`candles.set_bar_flags(times, filled=..., clipped=...)`を呼び、色は`candles.apply_options(filled_bar_color="#78909c")`で設定します。[Python](python.md#options-indicators-and-drawings)と[シリーズのオプション](series.md#series-options)を参照してください。

## リアルタイム更新

`update`は最後の足を変更するか、新しい足を追加します。

- 最後の足と同じ時刻：その足を置き換えます（形成中の足）。
- それより後の時刻：新しい足を追加します。
- それより前の時刻：例外を投げます。

```ts
// OHLC series
candles.update({ time: 1704067380000, open: 100.8, high: 101.2, low: 100.7, close: 101.0, volume: 300 });
// Value series
line.update({ time: 1704067380000, value: 101.0 });
```

足は`setData`のデータと同様に検証されます。最後の足の置き換えではメモリ確保は発生せず、チャートの再描画はアニメーションフレームごとに最大1回なので、約定のたびに`update`を呼んでも問題ありません。シリーズ上のインジケーターはインクリメンタルに更新されます。出来高シリーズはソースに追従するため、独自の`update`はありません。

変換シリーズ（Renko、Kagiなど）では、各`update`は入力の足またはティックであり、描画される要素がそれに応じて変わります。[足の変換](series.md#bar-transforms)を参照してください。

### まとめて追加する

末尾に複数の足を追加するには（再接続後にフィードから届いたまとまりなど）、`setData`が受け付ける任意の形式のカラムまたは行で`append`を呼びます。時刻は最後の足より後でなければならず、出来高カラムはシリーズが出来高を持つときに限り、必ず含める必要があります。そうでない場合は例外を投げ、データは元のままです。

```ts
candles.append([
  { time: 1704067440000, open: 101.0, high: 101.4, low: 100.9, close: 101.3, volume: 250 },
  { time: 1704067500000, open: 101.3, high: 101.5, low: 101.1, close: 101.2, volume: 180 },
]);
```

形成中の足には`update`を、まとめて届いた確定済みの足には`append`を使ってください。

## 過去データの読み込み

`prepend`は、`setData`が受け付ける任意の形式で、最初の足より前に古い足を挿入します。既存の足は論理インデックスを保ち、新しい足には負のインデックスが付きます（インデックス0の前の足は -1）。表示は動かず、シリーズ上のインジケーターは再計算されます。時刻は最初の足より前でなければならず、出来高のルールは`append`と同じです。

読み込むタイミングはチャートが知らせます。表示の左端より前に残っている足が1画面幅分より少なくなると、`needMoreHistory`イベントが送られます。`barsBeforeStart`は残っている足の数です（データより前の空白がすでに見えている場合は負）。イベントは最初の足ごとに1回送られます。`prepend`の後は最初の足が新しくなるため、ユーザーがそこに近づくまでスクロールすると再びイベントが届きます。

### 実例

```ts
import { FerroChart, type OhlcRow } from '@ferrochart/core';

// Your backend: up to `count` bars that end before `beforeMs`, oldest first.
declare function fetchBars(symbol: string, beforeMs: number, count: number): Promise<OhlcRow[]>;

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
chart.addVolumeSeries(candles);

const latest = await fetchBars('BTCUSD', Date.now(), 1000);
candles.setData(latest);
let first = Number(latest[0]?.time ?? Date.now());
let loading = false;
let exhausted = false;

chart.on('needMoreHistory', async () => {
  if (loading || exhausted) return;
  loading = true;
  try {
    // Only bars before the first one: an overlapping bar would make prepend throw.
    const older = (await fetchBars('BTCUSD', first, 1000)).filter((b) => Number(b.time) < first);
    if (older.length === 0) {
      exhausted = true; // the backend has nothing older
      return;
    }
    candles.prepend(older);
    first = Number(older[0].time);
  } finally {
    loading = false;
  }
});
```

`loading`フラグは、自分のコードからのリクエストの重複を防ぎます（イベント自体は同じ最初の足に対して繰り返されません）。その間もリアルタイムの足は`update`で送り続けられます。`prepend`と`update`は反対側の端で動作するためです。

## データの読み出し

| メソッド | 戻り値 |
|---|---|
| `series.dataAt(index)` | OHLCシリーズでは`{ open, high, low, close, volume }`（出来高カラムがなければ`volume`は`null`）、値シリーズでは`{ value }`。欠損またはデータの範囲外では`null` |
| `chart.timeAtIndex(index)` | 論理インデックスにある足の時刻（UTC ms）。データの範囲外では`null` |
| `chart.indexOfTime(time)` | ちょうどその時刻を持つ足の論理インデックス。なければ`null` |
| `chart.dataRange()` | `{ from, to }`：最初と最後の足の論理インデックス。データがなければ`null` |

小数のインデックスは最も近い足に丸められます。

```ts
const range = chart.dataRange();
if (range) {
  const last = candles.dataAt(range.to);              // the forming bar
  const t = chart.timeAtIndex(range.to);              // its time
  console.log(t, last);
}

const i = chart.indexOfTime(Date.UTC(2024, 0, 2, 9, 30));
if (i !== null) console.log(candles.dataAt(i));
```

`shift`を持つシリーズでは、`dataAt(index)`は`index`の位置に描画される点を返します。

## 論理インデックス

すべての足は論理インデックスを持ちます。表示範囲、クロスヘアの位置、クリックはこのインデックスで通知され、`setVisibleLogicalRange`や`logicalToCoordinate`が受け取るのもこのインデックスです。[時間軸とビューポート](time-scale.md)を参照してください。すべてのシリーズが時間軸を共有しているため、インデックス`i`はどのシリーズでも同じ時刻です。

`setData`の後、最初の足のインデックスは0です。足と足の間の位置が意味を持つ場所（表示の端、ポインター）ではインデックスは小数になり、足`i`は`i - 0.5`から`i + 0.5`までを占めます。

## 詳細度（LOD）

足はカラムとして保存されます。時刻の配列が1つ、始値の配列が1つ、という具合です。これにより読み込みが速く、メモリも小さく抑えられます。時刻インデックスと詳細度ピラミッドを含めて、1本あたり約58バイトです。

複数の足が1ピクセルを共有するまでズームアウトすると、FerroChartは事前計算されたピラミッド（各レベルで本数が半分になり、本当の高値と安値は保たれます）から集約した足を描画します。そのため1フレームで描画する要素の数は、データのサイズに関係なく画面の幅（物理ピクセルあたり約4個）で上限が決まります。

内部ではすべてのデータ変更がインクリメンタルです。レンダラーにはどの範囲が変わったかが伝えられ、その部分だけをアップロードします。

## 出来高

出来高シリーズはOHLCシリーズの出来高カラムを読みます：`chart.addVolumeSeries(candles)`。独自のデータは持たないので、ローソク足の`setData`と`update`に`volume`を渡してください。[シリーズの種類](series.md#volume)を参照してください。

## 例：模擬リアルタイムフィード

500本の過去データを持ち、250 msごとに新しい価格が届くチャートです。時計が分の境界を越えるたびに、新しい1分足が始まります。

```ts
import { FerroChart, randomWalk, type Bar } from '@ferrochart/core';

const chart = await FerroChart.create(canvas, { theme: 'dark' });
const candles = chart.addCandlestickSeries();
chart.addVolumeSeries(candles);

const interval = 60_000;
const now = Date.now();
const start = Math.floor(now / interval) * interval - 500 * interval;
const history = randomWalk(42, 500, start, interval); // deterministic demo data
candles.setData(history);

const n = history.close.length;
let bar: Bar = {
  time: Number(history.time[n - 1]),
  open: history.open[n - 1],
  high: history.high[n - 1],
  low: history.low[n - 1],
  close: history.close[n - 1],
  volume: history.volume?.[n - 1] ?? 0,
};

const timer = setInterval(() => {
  const t = Math.floor(Date.now() / interval) * interval;
  const price = bar.close * (1 + (Math.random() - 0.5) * 0.002);
  const size = Math.round(Math.random() * 10);
  if (t > Number(bar.time)) {
    // A new minute: open a new bar at the last close.
    bar = { time: t, open: bar.close, high: Math.max(bar.close, price), low: Math.min(bar.close, price), close: price, volume: size };
  } else {
    bar = { ...bar, high: Math.max(bar.high, price), low: Math.min(bar.low, price), close: price, volume: (bar.volume ?? 0) + size };
  }
  candles.update(bar); // same time: replaces, later time: appends
}, 250);

// When the chart goes away:
// clearInterval(timer); chart.destroy();
```

表示は最新の足からの距離（右オフセット）を保つため、ユーザーが最新のデータを見ていても過去にスクロールしていても、足が追加されるたびに内容が1本分左にスクロールします。`chart.scrollToRealtime()`で最新の足を表示に戻せます（[時間軸とビューポート](time-scale.md#right-offset)を参照）。
