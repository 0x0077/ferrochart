// Entry point of @ferrochart/core: the generated bindings, with
// `FerroChart.create` and `FerroPlot.create` loading the WebAssembly module
// on first use, so `await init()` is optional.
//
//   import { FerroChart } from '@ferrochart/core';
//   const chart = await FerroChart.create(canvas, { theme: 'dark' });
//
// Call `init()` yourself to load the module from another URL, or before
// using functions that need it without a chart (`randomWalk`, `new SyncGroup()`).

import init, { FerroChart, FerroPlot } from './ferrochart_wasm.js';

export * from './ferrochart_wasm.js';
export default init;

for (const Class of [FerroChart, FerroPlot]) {
  const create = Class.create;
  Class.create = async function (canvas, options) {
    await init(); // returns at once once loaded; concurrent calls share one load
    return create.call(this, canvas, options);
  };
}
