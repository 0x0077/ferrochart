import { categories, PRELUDE, preludeOf } from '@/catalog';

export const dynamic = 'force-static';

/** The chart catalog as data (/catalog.json): categories, their examples and
 * each example's code and prelude (`prelude`: the default one). Used by
 * tooling (card images, browser checks). */
export function GET() {
  return Response.json({
    prelude: PRELUDE,
    categories: categories.map(({ demos, ...c }) => ({ ...c, demos: demos.map((d) => ({ ...d, prelude: preludeOf(d) })) })),
  });
}
