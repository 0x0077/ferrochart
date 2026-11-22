// The chart library shown on the site, by use case. Each example is data;
// its code (catalog/build.ts) is what runs in the viewer and what the
// integration panel shows, so the two cannot drift apart.

import { ashares } from './ashares';
import { commodities } from './commodities';
import { custom } from './custom';
import { defi } from './defi';
import { equities } from './equities';
import { execution } from './execution';
import { finance } from './finance';
import { funds } from './funds';
import { indicators } from './indicators';
import { labs } from './labs';
import { meme } from './meme';
import { onchain } from './onchain';
import { options } from './options';
import { orderflow } from './orderflow';
import { perps } from './perps';
import { prediction } from './prediction';
import { price } from './price';
import { quant } from './quant';
import { rates } from './rates';
import { transforms } from './transforms';
import type { Category, Demo } from './types';

export type { Category, Demo };
export { PRELUDE, preludeOf } from './build';

export const categories: Category[] = [
  price,
  transforms,
  indicators,
  orderflow,
  execution,
  perps,
  onchain,
  defi,
  meme,
  equities,
  ashares,
  options,
  rates,
  commodities,
  funds,
  quant,
  finance,
  prediction,
  custom,
  labs,
].filter((c) => c.demos.length > 0);

export const allDemos: Demo[] = categories.flatMap((c) => c.demos);

export function findDemo(id: string): Demo | undefined {
  return allDemos.find((d) => d.id === id);
}

/** Groups of a category's demos in order of first appearance. */
export function groupsOf(demos: Demo[]): { name: string | null; demos: Demo[] }[] {
  const out: { name: string | null; demos: Demo[] }[] = [];
  for (const d of demos) {
    const name = d.group ?? null;
    let g = out.find((x) => x.name === name);
    if (!g) out.push((g = { name, demos: [] }));
    g.demos.push(d);
  }
  return out;
}
