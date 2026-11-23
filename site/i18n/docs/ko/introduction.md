<!-- source: f6f2d1d064 -->
# 소개

FerroChart는 Rust로 작성된 오픈 소스 금융 차트 엔진입니다. 하나의 코어가
모든 플랫폼을 구동합니다. 브라우저에서는 WebAssembly(WebGPU 또는 WebGL2)로,
네이티브 애플리케이션에서는 wgpu(Metal, Vulkan, DirectX 12)로 동작하며,
Python과 C에서도 사용할 수 있고 서버에서 헤드리스 렌더링도 할 수 있습니다.

FerroChart가 목표로 하는 것은 다음과 같습니다.

- **대용량 데이터.** 1,000만 개의 봉을 수십 밀리초 만에 불러오고, 200개일 때와
  똑같이 부드럽게 이동합니다. 프레임당 작업량은 데이터 크기가 아니라 화면에 따라 결정됩니다.
- **실시간 업데이트.** 마지막 봉을 업데이트하는 비용은 코어에서 1마이크로초에
  한참 못 미치며, 바뀐 부분만 다시 그립니다.
- **어디서나 같은 모습.** 모든 백엔드가 동일한 프레임 기술(description)을 바탕으로
  그리고, 동일한 골든 이미지로 검증됩니다.

## 한눈에 보는 아키텍처

```text
 your app ──► Chart (ferrochart-core) ──► Frame ──► renderer ──► pixels
              data, scales, layout,        plain       wgpu (GPU) / tiny-skia (CPU)
              interaction, LOD             draw list
```

- `ferrochart-core`는 데이터와 모든 차트 로직(스케일, 눈금, 레이아웃, 입력
  처리)을 담당합니다. GPU, 창, 폰트, 비동기 의존성이 없으므로
  `wasm32-unknown-unknown`을 포함해 어디서나 실행됩니다.
- 코어는 프레임마다 `Frame`을 생성합니다. `Frame`은 그리기 명령의 평평한
  목록입니다. 시리즈 데이터는 여기에 복사되지 않으며, 렌더러가 컬럼을 직접 읽고
  바뀐 부분만 업로드합니다.
- 렌더러는 프레임을 픽셀로 바꿉니다. 주 렌더러는 `ferrochart-render-wgpu`이며,
  `ferrochart-render-cpu`는 같은 프레임을 CPU에서 그립니다.
- 바인딩은 이 모두를 각 플랫폼에 맞게 감쌉니다. `ferrochart-wasm`(npm),
  `ferrochart-winit`(데스크톱), `ferrochart-py`(PyPI), `ferrochart-ffi`
  (C 라이브러리)가 있습니다.

## 어디서부터 시작할까

| 사용 환경 | 읽을 문서 |
|---|---|
| 웹 페이지의 JavaScript 또는 TypeScript | [웹](web.md) |
| React, Vue 또는 다른 프레임워크 | [React와 Vue](frameworks.md) |
| 데스크톱의 Rust | [데스크톱](desktop.md) |
| Python, Jupyter 또는 pandas | [Python](python.md) |
| C, C++, C# 또는 다른 네이티브 언어 | [C, C++, C#](native.md) |
| 서버에서 이미지 생성 | [오프스크린](offscreen.md) |
