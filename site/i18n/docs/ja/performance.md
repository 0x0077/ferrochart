<!-- source: 7e182ce114 -->
# パフォーマンス

## 目標

| | シナリオ | 目標 |
|---|---|---|
| P1 | `set_data`、1000万本の足、LODピラミッドを含む | < 300 ms |
| P2 | 足1本あたりのメモリ（OHLCV、定常状態） | < 64バイト |
| P3 | パン／ズームのフレームのコアCPU時間 | < 0.5 ms、データサイズに依存しない |
| P4 | パン／ズームのフレーム、エンドツーエンド | < 8 ms |
| P5 | `update_last` | < 20 µs |
| P6 | 16チャート × 100万本、クロスヘア連動 | 安定して60 fps |
| P7 | WebAssemblyパッケージ（gzip） | ≤ 1.5 MB |

## 実測値

Apple M4 Max、リリースビルド：

| | 実測値 |
|---|---|
| P1 | 73 ms |
| P2 | 58バイト |
| P3 | 19 µs（100万本でも1000万本でも同じ） |
| P4 | p99 2.2 ms（パン）、2.9 ms（ズーム）、3.2 ms（1000万本すべてを表示してパン）— 3200 × 1800 px、Metal |
| P5 | 0.3 µs（30種類のインジケーターすべてを使った場合17.1 µs） |
| P6 | 1フレームあたりp50 1.49 ms、p95 2.76 ms |
| P7 | 1.4 MB |

## ブラウザーでの比較

Chrome上で、FerroChart、lightweight-charts、Velaに同じデータ、同じページ、同じマウス入力を与えました（`examples/compare`）。100万本の場合：

| | 読み込み → 最初の描画 | 10万本を表示してパン（p95フレーム間隔／フレーム落ち） |
|---|---:|---:|
| FerroChart | 43 ms | 9.3 ms / 0 % |
| lightweight-charts 5.3.0 | 1603 ms | 74.9 ms / 36 % |
| Vela 0.10.13 | 77 ms | 該当なし — そこまでズームアウトできない |

## 再現方法

```bash
cargo bench -p ferrochart-core                       # P1, P3, P5
cargo bench -p ferrochart-bench --bench memory        # P2
cargo bench -p ferrochart-bench --bench frame_time    # P4 (needs a GPU)
cargo bench -p ferrochart-bench --bench sync          # P6 (needs a GPU)
cd examples/compare && npm install && node bench.mjs  # browser comparison
```
