// Node module hook for demo-run.mjs: lets `node --experimental-strip-types`
// import the site's TypeScript catalog, whose imports omit the extension.
export async function resolve(specifier, context, next) {
  if (specifier.startsWith('.') && !/\.[cm]?[jt]s$/.test(specifier)) {
    try { return await next(`${specifier}.ts`, context); } catch { /* fall through */ }
  }
  return next(specifier, context);
}
