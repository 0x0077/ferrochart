<!-- source: 29952e0395 -->
# JavaScript API 개요

패키지 `@ferrochart/core`. 이 페이지에서는 모듈을 로드하는 방법과 API 전반에
적용되는 규칙을 다룹니다. 모든 클래스, 메서드, 타입을 담은 전체 레퍼런스는
패키지의 TypeScript 선언에서 생성됩니다:
[JavaScript API 레퍼런스](https://ferrochart.com/docs/api).

## 로드

```ts
import { FerroChart } from '@ferrochart/core';

const chart = await FerroChart.create(canvas, { theme: 'dark' });
```

`FerroChart.create`와 `FerroPlot.create`는 처음 호출될 때 WebAssembly 모듈을
로드하며, 동시에 호출해도 한 번만 로드합니다. 차트를 만들기 전에 모듈이 필요하거나
(`randomWalk`, `new SyncGroup()`, `indicatorNames()`), `.wasm` 파일을 다른
URL에서 로드하려면 기본 내보내기인 `init`을 직접 호출하세요.

```ts
import init, { SyncGroup } from '@ferrochart/core';

await init();                                     // next to the module (default)
await init({ module_or_path: '/assets/ferrochart_wasm_bg.wasm' });
const group = new SyncGroup();
```

`init`은 몇 번이든 호출할 수 있으며, 처음 로드한 이후에는 즉시 반환합니다.

### TypeScript

타입 선언이 포함되어 있습니다. 선언에서 `Symbol.dispose`를 사용하므로
`compilerOptions.lib`에 `"ESNext.Disposable"`을 추가하거나, 흔히 쓰는
`skipLibCheck: true`를 유지하세요.

### 번들러와 프레임워크

패키지는 자신의 URL을 기준으로 `.wasm` 파일을 가져오는 ES 모듈이며
(`new URL('…_bg.wasm', import.meta.url)`), Vite, webpack 5, Rollup, esbuild가
이를 이해합니다. 차트는 브라우저에서만 만드세요. React에서는 effect를, Vue에서는
`onMounted`를 사용하고, 해당 컴포넌트는 서버 렌더링에서 제외하세요
([React와 Vue](frameworks.md) 참고).

## 클래스

| 클래스 | 생성 방법 | 설명 |
|---|---|---|
| [`FerroChart`](https://ferrochart.com/docs/api/FerroChart) | `FerroChart.create(canvas, options?)` | `<canvas>` 위의 금융 차트: 패널, 시리즈, 축, 크로스헤어, 드로잉 |
| [`Series`](https://ferrochart.com/docs/api/Series) | `chart.addSeries`, `addCandlestickSeries`, `addVolumeSeries`, `addTransformSeries`, 오더플로우 `add*` | 데이터 시리즈 하나 |
| [`Indicator`](https://ferrochart.com/docs/api/Indicator) | `chart.addIndicator` | 지표와 그 출력 시리즈 |
| [`PriceLine`](https://ferrochart.com/docs/api/PriceLine) | `series.createPriceLine` | 특정 가격의 수평선 |
| [`Drawings`](https://ferrochart.com/docs/api/Drawings) | `chart.drawings` | 차트의 드로잉과 드로잉 도구 |
| [`SyncGroup`](https://ferrochart.com/docs/api/SyncGroup) | `new SyncGroup()` | 여러 차트의 크로스헤어, 표시 범위, 드로잉을 연결 |
| [`FerroPlot`](https://ferrochart.com/docs/api/FerroPlot) | `FerroPlot.create(canvas, options?)` | 범용 x/y 플롯(숫자, 카테고리, 날짜) |
| [`Mark`](https://ferrochart.com/docs/api/Mark), [`Axis`](https://ferrochart.com/docs/api/Axis), [`Rule`](https://ferrochart.com/docs/api/Rule), [`Label`](https://ferrochart.com/docs/api/Label) | `FerroPlot`의 메서드 | 플롯의 구성 요소 |

## 규칙

### 시간

시간은 **UTC 밀리초**입니다. 열 데이터(`setData`의 `time`)는 `number[]`,
`Float64Array`, `BigInt64Array`를 받습니다. 그 밖의 모든 곳에서(행, `update`,
마커, 드로잉 앵커, `indexOfTime`) 단일 시간은 `number`, `bigint`, `Date` 중
하나일 수 있습니다. 시간은 엄격하게 증가해야 합니다. 이벤트와 getter는 시간을
`number`로 반환합니다(데이터 범위 밖이면 `null`). 시간 축 레이블은 UTC입니다.

### 논리 인덱스

봉은 **논리 인덱스**로 지정합니다. 초기 데이터의 첫 번째 봉이 0, 다음이 1과 같은
식입니다. 나중에 앞쪽에 추가된 봉은 음수 인덱스를 받으므로, 한 번 부여된 인덱스는
바뀌지 않습니다. 표시 범위, 크로스헤어, 변환 메서드(`coordinateToLogical`,
`timeAtIndex`, `indexOfTime`)가 이를 사용합니다. 소수 값은 봉 중심 사이의
위치입니다.

### 좌표

위치는 DOM 이벤트와 마찬가지로 캔버스 왼쪽 위 모서리로부터의 **CSS 픽셀**입니다.
캔버스 크기는 CSS로 정하며, 차트는 백킹 스토어를 CSS 크기 × `devicePixelRatio`로
설정하고 크기 변화를 스스로 따라갑니다.

### 색상

`Color`는 CSS 색상 문자열입니다: `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`,
`rgba()` 또는 `'transparent'`. 옵션에서 `null`은 테마의 기본값을 뜻합니다.

### 옵션 객체

옵션은 일반 객체입니다. `applyOptions`와 비슷한 메서드는 전달한 키만 바꾸며,
생략한 키는 값을 유지합니다. `series.options()` 같은 getter는 모든 키를
반환합니다(`Required<…>`).

### 오류

메서드는 문제를 명시한 메시지와 함께 `Error`를 던집니다(알 수 없는 지표 이름,
유효하지 않은 매개변수, 길이가 다른 열, 시간이 증가하지 않는 봉, 알 수 없는 id).
차트는 바뀌지 않습니다. 요청한 GPU API를 쓸 수 없으면 `FerroChart.create`가
reject됩니다. Canvas 2D 대체 경로는 없으며, `renderer: 'auto'`는 WebGPU를 먼저,
그다음 WebGL2를 시도합니다.

### 이벤트

`chart.on(name, callback)`으로 구독하고 `chart.off(name, callback)`으로 구독을
해제합니다. 콜백 매개변수의 타입은 이벤트 이름으로부터 정해집니다. 이벤트는 입력
핸들러 안이 아니라 이벤트를 만든 프레임 이후에 전달됩니다. 콜백이 던진 예외는
(`reportError`를 통해) 콘솔에 보고되며 나머지 콜백은 그대로 실행됩니다.
[크로스헤어와 이벤트](crosshair-events.md)를 참고하세요.

### 수명과 메모리

차트는 캔버스와 window를 감시하며 스스로 다시 그립니다. 페이지에 있는 동안에는
차트에 대한 참조를 유지하고, 제거할 때는 `chart.destroy()`를 호출하세요. 리스너를
제거하고 GPU 메모리를 해제합니다. `destroy` 없이 버려진 차트는 가비지 컬렉션되어
반응을 멈출 수 있습니다.

API의 모든 객체는 WebAssembly 메모리를 차지합니다. 차트가 반환한 객체(`Series`,
`Indicator`, `PriceLine`)는 차트와 함께 해제되며, `free()`나 `using` 선언으로 더
일찍 해제할 수 있습니다. 시리즈 제거(`remove()`)는 별개의 동작으로, 차트에서
시리즈를 없애는 것입니다.

### 캔버스에서 처리하는 입력

포인터(마우스, 펜, 터치), 휠, 키보드(캔버스에 `tabindex="0"`이 지정됨),
더블클릭, 트랙패드 핀치(Chrome과 Firefox에서는 Ctrl+휠, Safari에서는 제스처
이벤트), 그리고 차트가 직접 인식하는 두 손가락 터치입니다. 캔버스 스타일에는
`touch-action: none`이 지정되며, `touchScroll: 'page'`일 때는 `pan-y`가
지정됩니다([터치스크린](web.md#touch-screens) 참고). 각 제스처의 동작은
[제스처와 키보드](interaction.md)를 참고하세요.

## 테스트 데이터

`randomWalk(seed, n, startMs, intervalMs)`는 결정적인 OHLC 열을 반환하며, Rust
생성기 `ferrochart_core::testing::random_walk`와 결과가 같습니다. 예제와 데모가
이를 사용합니다.

```ts
import init, { randomWalk } from '@ferrochart/core';

await init();
const bars = randomWalk(42, 10_000, Date.UTC(2024, 0, 1), 60_000);
```
