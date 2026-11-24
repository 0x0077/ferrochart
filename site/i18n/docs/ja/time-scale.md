<!-- source: 80fd5ef585 -->
# 時間軸とビューポート

時間軸は、どの足を表示するか、足をどの幅で描くかを決めます。時間軸はすべてのペインで共有されます。この章では、足の間隔、右オフセット、表示範囲、ピクセル・論理インデックス・時刻の相互変換、表示の変化を通知するイベントを扱います。

## 論理インデックス

時間軸は時刻ではなく論理インデックスで動作します。`setData`の後、最初の足のインデックスは0、次が1、という具合で、足`i`は`i`を中心に`i − 0.5`から`i + 0.5`までを占めます。足と足の間やデータの範囲外の位置は、小数や範囲外のインデックスになります。時刻との相互変換には`timeAtIndex`と`indexOfTime`を使います（[データ](data.md#reading-data-back)を参照）。

インデックスは時間ではなく足を数えます。足のない週末や取引停止は場所を取りません。

## 足の間隔

足の間隔とは、隣り合う2本の足の中心間の距離で、CSSピクセル単位です。ズームするとこの値が変わります。

```ts
console.log(chart.barSpacing); // 6 on a new chart
chart.setBarSpacing(12);        // zoom in
```

| | 値 |
|---|---|
| デフォルト | 6 CSS px |
| 最小 | 0.5 CSS px。すべてのデータを一度に表示するのに必要ならそれ以下（プロット幅の90 % を足の本数で割った値） |
| 最大 | `maxBarSpacing`。デフォルトは50 CSS px |

`setBarSpacing`は値をこの範囲に収めます。正の有限の数でない値は無視されます。右端の足は同じ位置に留まります。

最大の間隔はチャートの作成時に設定します。

```ts
const chart = await FerroChart.create(canvas, { maxBarSpacing: 200 });
```

`maxBarSpacing`は1以上の数でなければならず、そうでない場合`create`はrejectされます。1本の足に広い場所が必要なときは大きくしてください。フットプリントの数値には60 px以上、TPOの文字には数百pxが必要です（[オーダーフロー](orderflow.md)を参照）。

## 右オフセット

右オフセットは、最後の足とプロットの右端との間の空白で、足の本数単位です。小数や負の値も取れます（負の場合、最後の足は右端より先にあり、過去のデータを見ている状態はこのように表されます）。

```ts
console.log(chart.rightOffset); // 3 on a new chart
chart.setRightOffset(10);        // more room for the forming bar
chart.scrollToRealtime();        // back to the latest bar, with the default offset of 3
```

`setRightOffset`は少なくとも1本の足が画面に残るようにします。その範囲外の値は丸め込まれ、有限でない値は無視されます。

表示は右オフセットによって最新の足に固定されています。`update`や`setData`で末尾に足が追加されても右オフセットは変わらないため、新しい足1本ごとに内容が1本分左へ移動します。ユーザーが最新のデータを見ている間は新しい足がスクロールして現れ、過去にスクロールしている場合は、見ている足も左へ移動します。`scrollToRealtime()`で最新の足に戻ります。

## 表示範囲

`visibleLogicalRange()`は、プロットの左端と右端の論理インデックスを小数で返します。

```ts
const { from, to } = chart.visibleLogicalRange();
const visibleBars = to - from;
```

`setVisibleLogicalRange(from, to)`は、間隔の制限が許す範囲で、ちょうどその範囲をプロット全体に表示します（許さない場合は右端が維持されます）。

```ts
// The last 100 bars.
const data = chart.dataRange();
if (data) chart.setVisibleLogicalRange(data.to - 99.5, data.to + 0.5);
```

`from`は`to`より小さく、どちらも有限でなければなりません。そうでない場合は例外を投げ、表示は変わりません。`create`の直後、最初のフレームより前に呼ぶこともでき、その場合はプロットの幅がわかった時点で範囲が適用されます。それより前にパンやズームをすると取り消されます。

`fitContent()`は、すべての足を左端から右端まで、右オフセットなしで表示します。足が少ない場合は、最大の足間隔のために左側に空白が残ることがあります。`setVisibleLogicalRange`と同様に、プロットの幅がわかるまで待ちます。

`dataRange()`は最初と最後の足のインデックスを返し（データがなければ`null`）、データを基準にした範囲の計算に使えます。

```ts
const data = chart.dataRange();
if (data) {
  // The first 200 bars.
  chart.setVisibleLogicalRange(data.from - 0.5, data.from + 199.5);
}
```

### 時刻による範囲指定

時刻は`indexOfTime`でインデックスに変換します。これはちょうどその時刻を持つ足を必要とします。任意の時刻に最も近い足を探すには、自分で保持している時刻のコピーを検索してください。

```ts
function showTimes(fromMs: number, toMs: number) {
  const a = chart.indexOfTime(fromMs);
  const b = chart.indexOfTime(toMs);
  if (a !== null && b !== null && a < b) chart.setVisibleLogicalRange(a - 0.5, b + 0.5);
}
```

## ピクセル、インデックス、時刻

| メソッド | 変換 |
|---|---|
| `logicalToCoordinate(index)` | 論理インデックス → 足の中心のx。canvasの左端からのCSS px |
| `coordinateToLogical(x)` | x（canvasの左端からのCSS px）→ 論理インデックス（小数） |
| `timeAtIndex(index)` | 論理インデックス → その足の時刻（UTC ms）。データの範囲外では`null`。最も近い足に丸めます |
| `indexOfTime(time)` | 時刻 → ちょうどその時刻を持つ足の論理インデックス。なければ`null` |

座標は直前のフレームのレイアウトを基準にしているため、最初のフレームの後に（たとえばイベントハンドラー内で）使ってください。x座標はどのペインでも同じです。

```ts
// The bar under a point of the canvas.
canvas.addEventListener('dblclick', (e) => {
  const rect = canvas.getBoundingClientRect();
  const index = Math.round(chart.coordinateToLogical(e.clientX - rect.left));
  const time = chart.timeAtIndex(index);
  if (time !== null) console.log(new Date(time).toISOString(), candles.dataAt(index));
});

// Position an HTML element over a bar.
function placeOverBar(el: HTMLElement, time: number) {
  const index = chart.indexOfTime(time);
  if (index === null) return;
  el.style.left = `${chart.logicalToCoordinate(index)}px`;
}
```

クロスヘアとクリックについては、イベントがすでに論理インデックス、時刻、価格を持っています。[クロスヘアとイベント](crosshair-events.md)を参照してください。

## visibleRangeChangedイベント

`visibleRangeChanged`は、表示中の論理範囲が変わるたびに送られます。ジェスチャー、上記のメソッド、リサイズ、表示を動かすデータのいずれによる変化でも送られます。

```ts
chart.on('visibleRangeChanged', ({ from, to }) => {
  const a = chart.timeAtIndex(Math.ceil(from));
  const b = chart.timeAtIndex(Math.floor(to));
  label.textContent = a !== null && b !== null
    ? `${new Date(a).toISOString()} – ${new Date(b).toISOString()}`
    : '';
});
```

イベントはそれを生じたフレームの後に、最大でフレームごとに1回配信されます。素早くパンしている間は、途中のすべての範囲ではなく最新の範囲が届きます。チャートにデータがない間は何も送られません。リスナーは`chart.off('visibleRangeChanged', callback)`で削除します。

表示が最初の足から1画面分以内に近づくと、同じフレームで`needMoreHistory`も送られることがあります。[過去データの読み込み](data.md#loading-older-history)を参照してください。

### 再読み込みをまたいで表示を保つ

`prepend`で古い足を読み込んでも、すべての論理インデックスと表示は保たれます。`setData`は足に0から番号を振り直すため、論理インデックスは変わりますが、時刻は変わりません。データを置き換えた後（たとえば同じタイムスタンプを持つ別の銘柄）に表示を復元するには、表示を時刻として保存してください。

```ts
function saveView() {
  const { from, to } = chart.visibleLogicalRange();
  return { from: chart.timeAtIndex(from), to: chart.timeAtIndex(to) };
}

function restoreView(view: { from: number | null; to: number | null }) {
  if (view.from === null || view.to === null) return chart.fitContent();
  const a = chart.indexOfTime(view.from);
  const b = chart.indexOfTime(view.to);
  if (a !== null && b !== null && a < b) chart.setVisibleLogicalRange(a, b);
}
```

足の間隔と右オフセットを描画やペインのレイアウトと一緒に保存するには、`saveState`を使います（[状態の保存](state.md)を参照）。複数のチャートを同じ範囲に保つには、[チャートの同期](sync.md)を参照してください。

## ユーザーのジェスチャー

次の入力で時間軸が変わります（一覧表は[ジェスチャーとキーボード](interaction.md)を参照）。

| 入力 | 効果 |
|---|---|
| プロット内でドラッグ | パン（動かしながら離すと慣性スクロール） |
| ホイール／トラックパッドのスクロール | ポインターを中心に時間方向にズーム |
| 横スクロールまたはShift + ホイール | パン |
| ピンチ（トラックパッド、2本指） | ズーム |
| 時間軸をドラッグ | 足の間隔を変更 |
| 時間軸をダブルクリック | 足の間隔をリセット |
| ← → | 10 % パン |
| ↑ ↓ | ズーム |
| Home / End | 最初／最新の足へジャンプ |

ズームしてもポインターの下の足は同じ位置に留まります。変化のたびに`visibleRangeChanged`が送られます。
