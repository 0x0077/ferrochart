<!-- source: 7f51672705 -->
# テーマとスタイル

この章では、チャートの見た目と、ページへの組み込み方を扱います。テーマ、色、軸と余白、操作、クロスヘア、多数の小さなチャート、画像出力、帰属表示ロゴ、イントロアニメーション、GPUレンダラー、サイズ設定、後片付けです。特に記載がない限り、ここでの内容はすべて`FerroPlot`にも当てはまります。

## テーマ

テーマはチャートの作成時に選びます。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { theme: 'dark' }); // 'light' (default) or 'dark'
```

テーマは、背景、グリッド、テキスト、軸、クロスヘアの色、ローソク足の陽線・陰線のデフォルト色、シリーズ、インジケーター、描画のデフォルト色を決めます。

テーマはオブジェクトでも指定できます。`base`テーマ（デフォルトは`'light'`）の上に重ねるCSSカラーで、指定しなかったキーは`base`から取られます。既存のチャートのテーマは`applyOptions`で変更でき、表示範囲とデータはそのまま保たれます。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, {
  theme: { base: 'dark', background: '#0b0d12', up: '#30a0f0', down: '#f09030', fontSize: 13 },
});
chart.applyOptions({ theme: 'light' });
```

キーは`background`、`text`、`grid`、`axisLine`、`up`、`down`、`upWick`、`downWick`、`volumeUp`、`volumeDown`、`crosshair`、`crosshairLabelBg`、`crosshairLabelText`、`line`、`fontSize`、`axisPadding`です。不明なキーはエラーになります。`chart.options().theme`で現在のテーマを読み出せます。

設定しなかった色や`null`に設定した色はテーマに従います。必要な箇所にだけ色を明示的に設定すれば、残りはどちらのテーマでも一貫した見た目に保たれます。

## シリーズの色

シリーズの色は`SeriesOptions`で指定します。作成時、または後から`series.applyOptions`で設定できます。

```ts
import { FerroChart } from '@ferrochart/core';

declare const chart: FerroChart;

const candles = chart.addCandlestickSeries({ color: '#26a69a', downColor: '#ef5350' });
const line = chart.addSeries('line', { color: '#2962ff', lineWidth: 2, lineStyle: 'dashed' });
const area = chart.addSeries('area', {
  color: '#2962ff',
  topFill: 'rgba(41, 98, 255, 0.4)',
  bottomFill: 'rgba(41, 98, 255, 0)',
});

line.applyOptions({ color: null }); // back to the theme's color
```

| オプション | 用途 |
|---|---|
| `color` | ライン、エリア、ベースライン（基準より上）シリーズのラインの色、陽線のローソク足とバー、ヒストグラムのバー |
| `downColor` | 陰線のローソク足とバー、ベースラインシリーズの基準より下のライン |
| `topFill` | エリアの上部の塗り、ベースラインの基準より上の塗り |
| `bottomFill` | エリアの下部の塗り、ベースラインの基準より下の塗り |
| `lineWidth` | CSS px単位のラインの太さ |
| `lineStyle` | `'solid'`、`'dashed'`または`'dotted'` |

エリアの塗りは`topFill`から`bottomFill`へのグラデーションです。ヒストグラムシリーズはバーごとの色（`setData`の`color`カラム）も受け付けます。すべてのシリーズオプションと、各シリーズの種類が描画する内容は[シリーズ](series.md)にあります。

他のオブジェクトにはそれぞれ色のオプションがあります。価格ライン（`createPriceLine({ color })`）、マーカー（マーカーごとの`color`）、2つのシリーズ間のバンド（`addBand(upper, lower, { fill, fillCrossed })`）、描画（`style: { line, fill, text }`）です。[シリーズ](series.md)、[マーカー](markers.md)、[インジケーター](indicators.md)、[描画](drawings.md)を参照してください。

## 色

色は次のいずれかの形式のCSSカラー文字列です（型`Color`）。

| 形式 | 例 |
|---|---|
| `#rgb`, `#rgba` | `'#f80'`, `'#f808'` |
| `#rrggbb`, `#rrggbbaa` | `'#ff8800'`, `'#ff880080'` |
| `rgb(r, g, b)` | `'rgb(255, 136, 0)'` |
| `rgba(r, g, b, a)` | `'rgba(255, 136, 0, 0.5)'`。アルファは0〜1 |
| `transparent` | `'transparent'` |

各チャンネルは0〜255で、カンマ、スペース、または`/`で区切ります。色名（`'red'`）、`hsl()`、パーセント指定は受け付け**ません**。呼び出しは例外を投げます（例：`'color': invalid color 'red'`）。

`options()`から読み出した色は`#rrggbb`で、不透明でない場合は`rgba(r, g, b, a)`で表されます。テーマに従う色は`null`として読み出されます。

ポイントごとの色のカラム（ヒストグラムのデータ、プロットのマーク）は、`0xRRGGBBAA`値の`Uint32Array`も受け付けます。大きなデータで文字列の生成を避けられます。

## 軸、グリッド、余白

アプリや表の中のチャートには軸がまったくないことがよくあります。`priceAxis`と`timeAxis`で軸の表示・非表示を、`grid`でグリッド線（`true`、`false`、または`{ horizontal, vertical }`）を切り替え、`padding`でプロット領域の周りに余白を取ります（CSS px、数値または`{ top, right, bottom, left }`）。非表示にした軸の領域はプロットに使われます。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, {
  priceAxis: false,
  grid: { vertical: false },
  padding: { top: 72, bottom: 40 },
});
chart.applyOptions({ timeAxis: false });
```

これらと次の2つの節のオプションは、`create`でも`applyOptions`でも使え、`chart.options()`で読み出せます。Pythonではsnake_caseのキーワード引数（`Chart(preset="sparkline")`、`chart.apply_options(price_axis=False)`）、Cでは`FerroChartAppearance`、`FerroChartInteraction`、`FerroChartCrosshairStyle`、`FerroChartTheme`構造体です。

## 操作

`interaction`はパン、ズーム、クロスヘアのオン・オフを切り替えます。`{ pan, zoom, crosshair }`、または`false`ですべてオフです。オフにした入力はチャートが使わないため、タッチスクリーンではページが通常どおりスクロール・ズームします。クリックは引き続き通知されます。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { interaction: { pan: false, zoom: false } });
```

## クロスヘアのスタイル

`crosshairStyle`はクロスヘアのどの部分を描くかを決めます。`vertical`と`horizontal`の線、軸上の`labels`、最初のシリーズの値の位置の`dot`（`{ radius, color }`、描かない場合は`null`。色のデフォルトはそのシリーズの色）です。縦線と点の組み合わせは、相場カードでよく使われるクロスヘアです。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, {
  crosshairStyle: { horizontal: false, labels: false, dot: { radius: 5 } },
});
```

## 多数の小さなチャート

`preset: 'sparkline'`は表のセルに置くチャートです。軸なし、グリッドなし、余白2、操作なしで、クロスヘアは縦線と点です。一緒に指定したオプションはその上に適用されます。

ブラウザが1ページで保持するWebGLコンテキストは16個程度で、チャートはそれぞれ自分のGPUデバイスを持ちます。小さなチャートを多数並べるページでは、プールから作成してください。1つのデバイス（WebGL2では1つのコンテキスト）を共有します。ビューポートの外にあるチャートは描画されず、スクロールして戻ったときに描かれます。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvases: HTMLCanvasElement[];

const pool = await FerroChart.createPool();
const charts = await Promise.all(canvases.map((c) => pool.create(c, { preset: 'sparkline' })));
// …
pool.destroy(); // destroys every chart of the pool
```

WebGL2では、プールのチャートは2Dコンテキストで自分のキャンバスに描くため、キャンバスにほかのコンテキストがあってはいけません。

## 画像出力

`chart.toBlob(type?, quality?)`は1フレームを描き、PNG（デフォルト）、WebP、JPEGの`Blob`として返します。`chart.toImageBitmap()`は`ImageBitmap`を返します。画像のピクセルサイズはキャンバスと同じで、キャンバスの外にある帰属表示ロゴは含まれません。`FerroPlot`にも同じメソッドがあります。

```ts
import { FerroChart } from '@ferrochart/core';

declare const chart: FerroChart;

const png = await chart.toBlob();
const jpeg = await chart.toBlob('image/jpeg', 0.9);
```

## 帰属表示ロゴ

デフォルトでは、最下段のペインの左下隅、時間軸のすぐ上に小さなFerroChartのロゴが表示されます。ロゴはferrochart.comへのリンクで、ポインターを重ねると名前が表示されます。

このロゴは好意によるもので、ライセンス上の要件ではありません。オフにしても構いません。`attributionLogo`を指定しない場合、ロゴは320 × 160 CSS px以上のキャンバスにだけ表示され、キャンバスのサイズがこの境界をまたぐと表示・非表示が切り替わります。`true`または`false`を指定する（または`setAttributionLogo`を呼ぶ）と固定されます。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { attributionLogo: false });

chart.setAttributionLogo(true);  // show it
chart.setAttributionLogo(false); // remove it
```

配置の仕組み：

- ロゴはcanvasの直後に挿入され、その隣に絶対配置される`<a>`要素です。canvasには描画されないため、スクリーンショットやエンジンがレンダリングした画像には現れません。
- レイアウトが変わると位置が更新されます。canvas自体と同様に、canvasのoffset parentを基準に配置されます。
- ロゴを追加する時点でcanvasに親要素がない場合、ロゴは追加されません。

`FerroPlot`にも同じオプションとメソッドがあり、ロゴはプロット領域の左下に置かれます。`attachSidePlot`でチャートに付けたプロットはロゴを失います。チャートのロゴが両方を代表するためです（[プロット](plots.md)を参照）。

## イントロアニメーション

`playIntro`はチャートを空の状態（グリッドと軸）で表示し、その後データを左から右へ描画します。ランディングページや、ユーザーが銘柄を開いたときなど、チャートが初めて表示される場面で使ってください。

```ts
import { FerroChart, randomWalk } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { theme: 'dark' });
chart.addCandlestickSeries().setData(randomWalk(1, 300, Date.UTC(2024, 0, 1), 3_600_000));

await chart.playIntro({ duration: 1200, delay: 200, pen: '#4cc9f0' });
// The animation has ended (or was ended early).
```

`IntroOptions`：

| オプション | 意味 | デフォルト |
|---|---|---|
| `duration` | データを左から右へ描画する時間（ms） | `900` |
| `delay` | 最初に空のチャートを表示する時間（ms） | `0` |
| `pen` | 描画の先端で光るラインの色。なしの場合は`null` | `null` |

動作：

- チャートは次のフレームを待たずにすぐ空の状態で表示されるので、全体のチャートが一瞬見えてしまうのを避けるには、`setData`の直後に`playIntro`を呼んでください。
- 価格ライン、値ラベル、クロスヘアは最後に表示されます。
- canvas上での押下、ホイール、ピンチ、ダブルクリック、キー操作でアニメーションは早めに終了します。チャート上でポインターを動かすだけでは終了しません。
- `chart.finishIntro()`でただちに終了します。
- `playIntro`をもう一度呼ぶと、実行中のアニメーションをまず終了させます。
- 返されるpromiseは、どのような終わり方でも（チャートが破棄された場合も）アニメーションの終了時にresolveされます。rejectされることはありません。無効なオプション（負または有限でない`duration`や`delay`、無効な`pen`の色）は呼び出し自体が例外を投げます。

`FerroPlot`では、`playIntro`はマークを左から右へ描画します。ルールのラベルとホバー時のハイライトは最後に表示されます。

## レンダラー

FerroChartはGPUで描画します。ブラウザーが対応していればWebGPUを、そうでなければWebGL2を使います。Canvas 2Dへのフォールバックはありません。

| `renderer`オプション | 動作 |
|---|---|
| `'auto'`（デフォルト） | WebGPU、次にWebGL2 |
| `'webgpu'` | WebGPUのみ |
| `'webgl2'` | WebGL2のみ |

要求したAPIがどれも利用できない場合（`'WebGPU is not
available'`、`'WebGL2 is not available'`、または両方を`;`でつないだもの）や、不明なオプション値の場合、`create`はrejectされます。使用中のAPIは`renderer`プロパティでわかります。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

try {
  const chart = await FerroChart.create(canvas);
  console.log(chart.renderer); // 'webgpu' or 'webgl2'
} catch (e) {
  canvas.replaceWith(Object.assign(document.createElement('p'), {
    textContent: 'This chart needs WebGPU or WebGL2.',
  }));
}
```

`'webgl2'`を強制すると、WebGPUを持つブラウザーでもフォールバックを簡単にテストできます。

## canvasのサイズ設定

canvasのサイズはCSSで指定します。FerroChartはcanvasのバッキングストア（`canvas.width`と`canvas.height`）をCSSサイズ × `devicePixelRatio`に自分で設定するため、高解像度の画面でもチャートが鮮明に表示されます。canvasの`width`と`height`を自分で設定しないでください。

```html
<div style="height: 60vh">
  <canvas id="chart" style="display: block; width: 100%; height: 100%"></canvas>
</div>
```

- `ResizeObserver`がcanvasのコンテンツボックスを監視します。CSSサイズが変わると、バッキングストアとレイアウトが追従し、チャートが再描画されます。リサイズ用のメソッドを呼ぶ必要はありません。
- `devicePixelRatio`が変わったとき（ブラウザーのズーム、ウィンドウを別の画面へ移動）も、バッキングストアがリサイズされます。
- バッキングストアは最小で1 × 1 pxです。作成時にチャートが非表示の要素内にある場合は、要素にサイズが付いた時点で描画されます。
- `display: block`を指定すると、インラインのcanvasの下にできる数ピクセルの隙間を避けられます。
- flexやgridのレイアウト内では、コンテナーがcanvasを縮められるよう、canvasに`min-height: 0`（または`min-width: 0`）を指定してください。

チャートの作成時、canvasにはキー入力を受け取れるよう`tabindex="0"`が（`tabindex`がない場合に）付けられ、タッチ操作でページではなくチャートをパン・ズームするよう`touch-action: none`が設定されます（`touchScroll: 'page'`では`pan-y`になり、縦スワイプはページで処理します。[タッチスクリーン](web.md#touch-screens)を参照）。さらに`outline: none`も設定されます。カーソルのスタイルは、ポインターがチャート上にある間に設定されます。

## 後片付け

```ts
import { FerroChart, FerroPlot } from '@ferrochart/core';

declare const chart: FerroChart;
declare const plot: FerroPlot;

chart.destroy();
plot.destroy(); // FerroPlot's equivalent
```

`destroy()`は次のことを行います。

- canvas上のDOMイベントリスナーと、`on`で追加したすべてのコールバックを削除します。
- チャートの同期グループから離脱します。
- 帰属表示ロゴを削除します。
- 保留中の`playIntro`のpromiseをresolveします。
- GPUリソースを解放します。

canvas要素はページに残ります。不要であれば自分で削除してください。`destroy()`の後はチャートオブジェクトを使用できず、その`Series`、`Indicator`などのハンドルに対する呼び出しは例外を投げます（`the FerroChart was
destroyed`）。表示しなくなったチャートは破棄してください。各チャートはGPUメモリを保持しており、ブラウザーは1ページが持てるWebGLコンテキストの数を制限しています。

ReactやVueでは、コンポーネントのマウント時にチャートを作成し、アンマウント時に破棄してください。[ReactとVue](frameworks.md)を参照してください。
