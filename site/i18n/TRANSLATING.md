# Translating the site

The site is in English (no prefix) and in four more languages, each under its
own path prefix:

| Locale | Prefix | Language |
|---|---|---|
| `zh` | `/zh` | 简体中文 |
| `zh-tw` | `/zh-tw` | 繁體中文（台灣用語） |
| `ja` | `/ja` | 日本語 |
| `ko` | `/ko` | 한국어 |

What is translated, and where it lives:

- **Interface text** (header, homepage, viewer, docs chrome): `lib/messages/<locale>.ts`.
- **Examples** (category titles and blurbs, group names, demo titles and
  descriptions): `i18n/catalog/<locale>.json`, same keys as `i18n/catalog/en.json`
  (generated from `catalog/*.ts` by `node scripts/i18n.mjs extract`).
- **User guide**: `i18n/docs/<locale>/<chapter>.md`, one file per chapter of
  `docs/book/src`, including `SUMMARY.md`.
- The JavaScript API reference is generated from the source and stays in English.

`node scripts/i18n.mjs check` lists what is missing or outdated: every
translation records a hash of the English it was made from (`"src"` in the
catalog files, the `<!-- source: … -->` first line of a chapter). After
updating a translation, run `node scripts/i18n.mjs stamp` — but delete the old
`src` / source line first, since `stamp` only fills in missing ones.

## Rules

1. **Never change code.** Fenced code blocks, inline `code`, API names
   (`addCandlestickSeries`, `Series.update`), file names, URLs, CLI commands
   and option names stay byte-for-byte identical. Comments inside code blocks
   stay in English.
2. **Keep the Markdown structure.** Same headings, same levels, same order
   (heading anchors are taken from the English chapter by position), same
   tables (same number of rows and columns), same lists, same links (translate
   only the link text; never the target). Keep blank lines between blocks.
3. **Product names stay as they are:** FerroChart, FerroPlot, FerroChart Pro,
   WebGPU, WebGL2, WebAssembly, wgpu, Metal, Vulkan, DirectX 12, Rust,
   Python, React, Vue, npm, crates.io, PyPI, lightweight-charts, mdBook,
   Apache-2.0. Ticker symbols (BTCUSD), units (ms, px, MB) and numbers stay too.
4. **Well-known financial terms** use the term traders in that market use
   (see the glossary); when a term is usually left in English (e.g. Renko,
   Kagi, VWAP, TPO, MVRV, SOPR, Ichimoku in some languages), leave it.
5. Write natural, concise technical prose for developers, not a word-for-word
   rendering. Keep the meaning exact: no added claims, no dropped caveats.
6. Style:
   - `zh`: 简体中文；中文与英文/数字之间加一个半角空格；全角标点。
   - `zh-tw`: 繁體中文，台灣用語（資料、程式、預設、檔案、物件、函式、執行緒、
     記憶體、介面、支援、效能、伺服器）；全形標點；中英文之間加半形空格。
   - `ja`: です・ます調。全角句読点（、。）。英単語との間にスペースは入れない。
   - `ko`: 합니다체. 영문·숫자와 띄어쓰기는 자연스럽게.

## Glossary

| English | zh | zh-tw | ja | ko |
|---|---|---|---|---|
| chart | 图表 | 圖表 | チャート | 차트 |
| candlestick | K 线 / 蜡烛图 | K 線 / 蠟燭圖 | ローソク足 | 캔들스틱 |
| bar (OHLC) | K 线 / 柱 | K 線 / 棒 | 足 / バー | 봉 / 바 |
| series | 系列 | 系列 | シリーズ | 시리즈 |
| indicator | 指标 | 指標 | インジケーター | 지표 |
| drawing tool | 画线工具 | 繪圖工具 | 描画ツール | 드로잉 도구 |
| order flow | 订单流 | 訂單流 | オーダーフロー | 오더플로우 |
| footprint | 足迹图 | 足跡圖 | フットプリント | 풋프린트 |
| order book / depth | 订单簿 / 深度 | 委託簿 / 深度 | 板 / 板情報 | 호가창 / 호가 |
| liquidity heatmap | 流动性热力图 | 流動性熱力圖 | 流動性ヒートマップ | 유동성 히트맵 |
| price scale | 价格轴 | 價格軸 | 価格スケール | 가격 축 |
| time scale | 时间轴 | 時間軸 | 時間軸 | 시간 축 |
| crosshair | 十字光标 | 十字游標 | クロスヘア | 크로스헤어 |
| pane | 窗格 | 窗格 | ペイン | 패널 |
| marker | 标记 | 標記 | マーカー | 마커 |
| renderer | 渲染器 | 渲染器 | レンダラー | 렌더러 |
| plugin | 插件 | 外掛 | プラグイン | 플러그인 |
| zoom / pan | 缩放 / 平移 | 縮放 / 平移 | ズーム / パン | 확대·축소 / 이동 |
| volume | 成交量 | 成交量 | 出来高 | 거래량 |
| open interest | 持仓量 | 未平倉量 | 建玉 | 미결제약정 |
| funding rate | 资金费率 | 資金費率 | 資金調達率 | 펀딩비 |
| liquidation | 清算 / 爆仓 | 清算 / 爆倉 | 清算 | 청산 |
| perpetual (perps) | 永续合约 | 永續合約 | 無期限先物 | 무기한 선물 |
| implied volatility | 隐含波动率 | 隱含波動率 | インプライド・ボラティリティ | 내재 변동성 |
| backtest | 回测 | 回測 | バックテスト | 백테스트 |
| drawdown | 回撤 | 回撤 | ドローダウン | 낙폭(드로다운) |
| synthetic data | 模拟数据 | 模擬資料 | 合成データ | 합성 데이터 |
| example | 示例 | 範例 | サンプル | 예제 |
| use case | 应用场景 | 應用情境 | ユースケース | 활용 사례 |
| incremental | 增量 | 增量 | インクリメンタル | 증분 |
