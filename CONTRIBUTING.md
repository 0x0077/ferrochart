# Contributing to FerroChart

Thank you for your interest in FerroChart. This guide explains how to propose changes
and what a pull request needs before it can be merged.

## Before you start

- **Discuss design changes first.** If your change affects the architecture, the
  public API or the performance targets, open an issue that proposes it before you
  write the code.
- **For larger changes, open an issue first** so the approach can be agreed before you
  spend time on it. Small fixes (typos, docs, obvious bugs) can go straight to a PR.

## Development setup

You need Rust 1.93 or newer (`rustup` recommended) and the `wasm32-unknown-unknown`
target:

```bash
rustup target add wasm32-unknown-unknown
```

Common commands:

```bash
scripts/check.sh   # run every check below, plus the MSRV check, before you commit
cargo fmt --all
cargo clippy --workspace --exclude ferrochart-py --all-targets -- -D warnings
cargo test --workspace --exclude ferrochart-py
cargo test -p ferrochart-core --features testing
cargo build -p ferrochart-core --target wasm32-unknown-unknown
```

`ferrochart-py` needs a Python interpreter, so it is excluded from the workspace
commands. It is built and tested with maturin and pytest.

## Rules that are never relaxed

1. `ferrochart-core` and `ferrochart-indicators` must not depend on GPU, windowing, font,
   time or async libraries. Dependencies are kept to a short allowed list; adding one
   needs agreement in an issue first.
2. The core only produces a `Frame`. Backends contain no chart logic (ticks, up/down
   colors, layout).
3. The public API never panics: no `unwrap`/`expect` outside tests. Methods that take
   ids return `Result` or `Option`.
4. Hot paths (`build_frame` for pan/zoom/crosshair, and `update_last`) perform zero heap
   allocations. This is enforced by counting-allocator tests.
5. Coordinates use the `Px`/`Css` types, never bare `f32`.
6. `transform_helpers` and the matching wgpu shader code keep the same f32 operation
   order. Changing one means changing the other.
7. Changes that affect rendered output must pass the golden image tests. Golden updates
   go in a separate commit that explains why they changed.
8. Changes that may affect performance include before/after benchmark numbers.

## Pull request checklist

- [ ] `cargo fmt`, `cargo clippy -D warnings` and all tests pass
- [ ] New public API has rustdoc with an example
- [ ] Types used in `Frame` are `Copy` and use `Span`/`IdxSpan` instead of `Range`
- [ ] New indicators: the definition is documented in `docs/book/src/indicators.md`, with
      reference-data tests and an "incremental == full" proptest
- [ ] Rendering changes are covered by golden images
- [ ] Hot-path changes have zero-allocation tests
- [ ] Performance-sensitive changes include benchmark data and do not regress by more
      than 10%

## Commits

- One commit does one thing.
- Use [Conventional Commits](https://www.conventionalcommits.org/) with the crate as
  scope, for example `feat(core): ...`, `fix(render-cpu): ...`, `perf(render-wgpu): ...`.
- Small, non-blocking decisions taken during implementation (naming, details the design
  does not cover) are noted in the commit message.

## License

By contributing, you agree that your contributions will be licensed under the
Apache-2.0 license, as described in the [README](README.md#license), without any
additional terms or conditions.
