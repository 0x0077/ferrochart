<!-- source: f3b74de278 -->
# Rust API

APIドキュメントはrustdocで生成されます。ローカルでは次のように実行します。

```bash
cargo doc --workspace --exclude ferrochart-py --no-deps --open
```

docs.rsで公開されているクレート：

| クレート | 内容 |
|---|---|
| [`ferrochart-core`](https://docs.rs/ferrochart-core) | `Chart`、データモデル、スケール、入力、`Frame`、`Renderer`トレイト、`transform_helpers` |
| [`ferrochart-render-wgpu`](https://docs.rs/ferrochart-render-wgpu) | `GpuContext`、`WgpuRenderer`（サーフェスまたはオフスクリーン） |
| [`ferrochart-render-cpu`](https://docs.rs/ferrochart-render-cpu) | `CpuRenderer`（tiny-skia） |
| [`ferrochart-text`](https://docs.rs/ferrochart-text) | `TextEngine`、同梱フォント |
| [`ferrochart-winit`](https://docs.rs/ferrochart-winit) | `run`、`ChartApp`、`InputConverter` |
| [`ferrochart-indicators`](https://docs.rs/ferrochart-indicators) | インクリメンタルなインジケーター |
| [`ferrochart-orderflow`](https://docs.rs/ferrochart-orderflow) | フットプリント、ヒートマップ、TPOシリーズ（[オーダーフロー](orderflow.md)） |

チャートの拡張（レイヤー、カスタムシリーズ、ネイティブGPUシリーズ、変換、描画ツール）については[プラグイン](plugins.md)で説明しています。

## どこから始めるか

- `ferrochart_core::Chart`：`ChartOptions`で作成し、`resize`、シリーズの追加、データの設定を行い、`handle_input`で`InputEvent`を渡し、`build_frame`を呼び、`drain_events`でイベントを読み取ります。
- `ferrochart_core::render::Renderer`：両方のレンダラーが実装しています。`render(chart.frame(), &chart.data_view())`を呼び出します。

## APIが従うルール

- **パニックしない。** 失敗しうるメソッドは`Result`を返し、idによる検索は`Result`または`Option`を返します。
- **ホットパスでアロケーションしない。** パン、ズーム、クロスヘアの移動（`build_frame`）と`update_last`は、ウォームアップ後は一切アロケーションしません。テストではカウント用アロケーターでこれを検証しています。
- **型付きの座標。** `Css`と`Px`（およびそれらを基にした点・サイズ・矩形の型）によってCSSピクセルとデバイスピクセルを区別します。デバイスピクセル比は`Chart::resize`に渡します。
- **コアは純粋。** `ferrochart-core`は標準ライブラリ以外に依存しません（許可されている唯一の例外は`smallvec`です）。GPU、ウィンドウ、フォント、時間、非同期のライブラリは使わず、`unsafe`もありません。
