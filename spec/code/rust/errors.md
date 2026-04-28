# Error Handling

Errors are part of the API. Treat them with the same care as any other type.

## Result Is The Default

- Every fallible function returns `Result<T, E>`.
- `Option<T>` represents the possible absence of a value, not failure.
- Do not use sentinel return values (`-1`, empty string) to signal errors.

## Use The `?` Operator

- Propagate errors with `?`. It is the default control flow for fallible code.
- Drop into explicit `match` only when each error branch needs different
  handling.

```rust
fn load_config(path: &Path) -> Result<Config, ConfigError> {
    let bytes = std::fs::read(path)?;
    let config = serde_json::from_slice(&bytes)?;
    Ok(config)
}
```

## Library Code: `thiserror`

- Define a domain error enum with `thiserror::Error`.
- Variants describe failure modes the caller may want to branch on.
- Implement `From` for inner errors so `?` works without manual mapping.

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file")]
    Io(#[from] std::io::Error),

    #[error("invalid config format")]
    Parse(#[from] serde_json::Error),
}
```

## Application Code: `anyhow`

- Binaries and entry points return `anyhow::Result<T>`.
- Add context with `.context("...")` whenever an error crosses a boundary the
  user cares about.

```rust
fn main() -> anyhow::Result<()> {
    let config = load_config(Path::new("config.json"))
        .context("failed to load config at startup")?;
    run(config)?;
    Ok(())
}
```

## No Panics In Production Paths

- `unwrap()` and `expect()` are allowed only when the invariant is impossible
  to violate at runtime — for example, parsing a compile-time constant.
- `panic!()` is reserved for unrecoverable bugs. User-triggered conditions
  always return `Result`.
- Never `unwrap` an I/O, parsing, or external call. Return the error instead.

## Error Messages

- All error messages are written in English.
- Messages describe what went wrong from the caller's perspective. Avoid
  leaking implementation details into user-facing text.
- Use `Display` for short, human-readable messages and `Debug` for detail when
  diagnostics are needed.

## Do Not Discard Results

- Never call a fallible function and ignore its return value.
- If discarding is intentional, write `let _ = fallible_call();` so the intent
  is explicit and reviewable.
