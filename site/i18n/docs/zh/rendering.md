<!-- source: 4f8ac0be0b -->
# 渲染的工作原理

## 循环

每种绑定都运行同一个循环：

```rust,ignore
if chart.needs_redraw() != Invalidation::None {
    chart.build_frame(now_ms, &renderer.measurer()); // &mut chart, ends here
    renderer.render(chart.frame(), &chart.data_view())?; // two shared borrows
}
```

没有任何变化时，`needs_redraw` 一直为 `None`，因此空闲的图表没有任何开销。构建一帧大约需要 20 µs，与数据量无关。

## 帧

一个 `Frame` 有三层——背景层（网格）、主层（系列、坐标轴）和叠加层（十字光标）——每层都是一个带修订号的扁平绘制命令列表。系列不会被展开成矩形：帧描述的是*用这个变换绘制这个系列的可见范围 i..j*，渲染器直接读取数据列。

当只有十字光标移动时，只有叠加层的修订号会变化，GPU 渲染器会复用另外两层的缓存纹理。

## GPU 上的数据

wgpu 渲染器把系列数据保存在 GPU 缓冲区中，每块 4096 根 K 线。某个块第一次被绘制时上传，之后只上传发生变化的范围。平移和缩放只改变每个块的少数几个数值；不会重新上传任何数据。

价格以相对于每个块的参考价格的形式存储，因此即使价格很大而最小变动单位很小，GPU 上的 32 位浮点数也能保持完整精度。

## 处处相同的像素

坐标计算位于 `ferrochart_core::transform_helpers` 中，并在 WGSL 着色器中逐个运算地镜像实现，因此 CPU 和 GPU 渲染器的舍入完全相同。黄金图像测试会把两者都与同一组参考图像比较。（有一个微妙之处：GPU 编译器可能会把 `a * b + c` 融合成一条舍入方式不同的指令，所以变换被写成无法融合的 `(a + b) * c` 形式。）
