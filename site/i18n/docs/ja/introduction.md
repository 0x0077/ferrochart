<!-- source: f6f2d1d064 -->
# はじめに

FerroChartは、Rustで書かれたオープンソースの金融チャートエンジンです。ひとつのコアがすべてのプラットフォームを動かします。ブラウザではWebAssembly（WebGPUまたはWebGL2）、ネイティブアプリケーションではwgpu（Metal、Vulkan、DirectX 12）、さらにPythonとC、そしてサーバー上でのヘッドレスレンダリングにも対応します。

設計上の主な狙いは次のとおりです。

- **大量データ。** 1,000万本の足が数十ミリ秒で読み込まれ、200本のときと同じくらい滑らかにパンできます。フレームごとの処理量はデータ量ではなく画面に依存します。
- **リアルタイム更新。** 最後の足の更新にかかるコストはコア内で1マイクロ秒を大きく下回り、再描画されるのは変更された部分だけです。
- **どこでも同じ見た目。** すべてのバックエンドが同じフレーム記述から描画し、同じゴールデンイメージで検証されています。

## 1枚の図で見るアーキテクチャ

```text
 your app ──► Chart (ferrochart-core) ──► Frame ──► renderer ──► pixels
              data, scales, layout,        plain       wgpu (GPU) / tiny-skia (CPU)
              interaction, LOD             draw list
```

- `ferrochart-core`はデータとすべてのチャートロジック（スケール、目盛り、レイアウト、入力処理）を保持します。GPU、ウィンドウ、フォント、非同期処理への依存がなく、`wasm32-unknown-unknown`を含むあらゆる環境で動作します。
- フレームごとに`Frame`を生成します。これは描画コマンドのフラットなリストです。シリーズのデータはそこへコピーされず、レンダラーがカラムを直接読み取り、変更された部分だけをアップロードします。
- レンダラーはフレームをピクセルに変換します。主要なものは`ferrochart-render-wgpu`で、`ferrochart-render-cpu`は同じフレームをCPUで描画します。
- バインディングは、これらすべてを各プラットフォーム向けにラップします。`ferrochart-wasm`（npm）、`ferrochart-winit`（デスクトップ）、`ferrochart-py`（PyPI）、`ferrochart-ffi`（Cライブラリ）があります。

## どこから始めるか

| 使用環境 | 読むべき章 |
|---|---|
| WebページでのJavaScriptまたはTypeScript | [Web](web.md) |
| React、Vue、その他のフレームワーク | [React と Vue](frameworks.md) |
| デスクトップでのRust | [デスクトップ](desktop.md) |
| Python、Jupyter、pandas | [Python](python.md) |
| C、C++、C#、その他のネイティブ言語 | [C、C++、C#](native.md) |
| サーバー上での画像生成 | [オフスクリーン](offscreen.md) |
