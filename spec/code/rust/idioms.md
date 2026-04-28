# Rust Idioms

Write idiomatic Rust. Lean on the type system, ownership, and iterators instead
of fighting them. The rules below capture the patterns that make Rust code
predictable and easy to read.

## Naming

- `snake_case` for functions, variables, modules, and crates.
- `CamelCase` for types, traits, and enum variants.
- `SCREAMING_SNAKE_CASE` for constants and statics.
- Functions are verbs (`parse_config`, `start_stream`). Types are nouns
  (`Config`, `AudioStream`).
- Boolean variables and methods read as predicates (`is_ready`, `has_input`).

## Borrow By Default

- Function parameters take borrows (`&str`, `&[T]`, `&Path`) unless ownership
  must transfer.
- Return owned values only when the caller needs to own the result.
- `clone()` is a deliberate choice. Never use it to silence a borrow checker
  error without thinking through ownership.

```rust
fn render_label(name: &str, count: usize) -> String {
    format!("{name} ({count})")
}
```

## Mutability Is Opt-In

- `let` is the default. Use `let mut` only when the value is genuinely mutated.
- Prefer rebinding (`let x = transform(x);`) over `mut` when reassignment is
  clearer.
- Interior mutability (`Cell`, `RefCell`, `Mutex`) is a tool, not a workaround.
  Reach for it only when shared mutation is unavoidable.

## Iterators Over Manual Loops

- Use iterator chains (`map`, `filter`, `fold`, `collect`) for transformations.
- Drop into a `for` loop when the chain hurts readability or needs early exit
  with side effects.
- Avoid manual indexing (`for i in 0..v.len()`) when an iterator works.

```rust
let active_names: Vec<String> = users
    .iter()
    .filter(|u| u.is_active)
    .map(|u| u.name.clone())
    .collect();
```

## Type-Driven Design

- No stringly-typed APIs. Wrap domain values in newtypes (`struct UserId(u64)`)
  or enums.
- Use enums for closed sets of variants. Keep `match` exhaustive — avoid `_`
  catch-alls unless the variants are genuinely irrelevant.
- Prefer `Option<T>` over sentinel values like `-1` or empty strings.

```rust
enum Trigger {
    DoubleClap,
    Hotkey(String),
}

fn describe(trigger: &Trigger) -> &'static str {
    match trigger {
        Trigger::DoubleClap => "double clap",
        Trigger::Hotkey(_) => "hotkey",
    }
}
```

## Traits And Generics

- Keep traits small and focused on a single responsibility.
- Implement standard traits (`From`, `Into`, `AsRef`, `Default`, `Display`)
  whenever they make calls more natural.
- Introduce generics only when the same shape appears in two or more places.
  Do not pre-generalize.

## Visibility

- Start with `pub(crate)`. Promote to `pub` only when an item is part of the
  external API.
- Struct fields are private by default. Expose them through accessors only when
  there is a reason to.

## Derives

- Derive `Debug`, `Clone`, `Default` when they make the type easier to use.
- Derive `PartialEq`, `Eq`, `Hash` only when value-equality has real meaning
  for the type.
- Avoid blanket-deriving traits the type does not actually need.
