<!-- source: 4f8ac0be0b -->
# 渲染的運作方式

## 迴圈

每個綁定都執行相同的迴圈：

```rust,ignore
if chart.needs_redraw() != Invalidation::None {
    chart.build_frame(now_ms, &renderer.measurer()); // &mut chart, ends here
    renderer.render(chart.frame(), &chart.data_view())?; // two shared borrows
}
```

沒有任何變化時，`needs_redraw` 會維持 `None`，因此閒置的圖表不耗費任何資源。建構一個影格約需 20 µs，與資料量無關。

## 影格

`Frame` 有三個圖層——背景（格線）、主圖層（系列、座標軸）與覆蓋層（十字游標）——每一層都是一份帶有修訂編號的扁平繪製指令清單。系列不會被展開成矩形：影格描述的是「*以此轉換繪製這個系列的可見範圍 i..j*」，由渲染器直接讀取資料欄。

當只有十字游標移動時，只有覆蓋層的修訂編號會改變，GPU 渲染器會重複使用另外兩層的快取紋理。

## GPU 上的資料

wgpu 渲染器把系列資料以每塊 4096 根 K 棒的區塊保存在 GPU 緩衝區中。區塊第一次被繪製時才上傳，之後只上傳變更的範圍。平移與縮放只會改變每個區塊的幾個數字；不需要重新上傳任何資料。

價格以相對於每個區塊之參考價格的形式儲存，因此即使是價格很大而 tick 很小的情況，GPU 上的 32 位元浮點數仍能保有完整精度。

## 處處相同的像素

座標運算位於 `ferrochart_core::transform_helpers`，並在 WGSL shader 中逐一運算地對應實作，因此 CPU 與 GPU 渲染器的捨入方式完全相同。黃金圖像測試會將兩者與相同的參考影像比對。（一個細節：GPU 編譯器可能會把 `a * b + c` 融合成一道捨入方式不同的指令，因此轉換被寫成無法融合的 `(a + b) * c`。）
