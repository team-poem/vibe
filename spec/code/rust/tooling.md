# Tooling And Review

Use the standard Rust toolchain. The same checks should pass on every machine
and in CI.

## Formatting

- `cargo fmt` is mandatory. CI rejects branches that fail `cargo fmt --check`.
- Do not hand-tune formatting that disagrees with `rustfmt`.

## Linting

- Run `cargo clippy --all-targets --all-features -- -D warnings`.
- A clippy warning either gets fixed or gets an `#[allow(...)]` attribute with
  a short comment explaining why the lint is wrong for this case.
- Treat `unused` warnings as bugs, not noise.

## Testing

- Co-locate unit tests in the module under test:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      // ...
  }
  ```

- Put integration tests in `tests/`. Each file there is its own crate.
- Public API gets doctest examples whenever the example clarifies usage.
- Tests are deterministic. No reliance on wall-clock time, real network, or
  ambient state unless the test explicitly owns that resource.

## Cargo Hygiene

- Pin dependencies to a major version (`"1"`, `"0.15"`). Avoid `"*"` and avoid
  pinning a full minor unless required.
- Enable only the cargo features you actually use.
- Remove unused dependencies as soon as they become unused.
- Release profile baseline:

  ```toml
  [profile.release]
  opt-level = 3
  lto = "thin"
  ```

## Review Checklist

Run through this before finishing any Rust change.

- [ ] `cargo fmt` and `cargo clippy -- -D warnings` both pass.
- [ ] No `unwrap()`, `expect()`, or `panic!()` outside places where the
      invariant is impossible to violate.
- [ ] Function parameters take borrows unless ownership must transfer.
- [ ] Error types match the layer: `thiserror` enums in libraries,
      `anyhow::Result` in binaries.
- [ ] No lock held across `await`, blocking I/O, or external callbacks.
- [ ] Visibility is the smallest that works. `pub(crate)` over `pub` whenever
      possible.
- [ ] Iterators replace manual loops where they read more clearly.
- [ ] No `Result` is silently discarded; intentional discards are written
      `let _ = ...`.
- [ ] Tests cover the change and run deterministically.

## Completion Step

This checklist is the required final review step before finishing Rust work.
