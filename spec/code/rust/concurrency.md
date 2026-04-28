# Concurrency

Rust gives strong concurrency guarantees, but only if you choose the right
shape. Default to the simpler patterns first.

## Prefer Message Passing

- When two threads need to coordinate, start with channels
  (`std::sync::mpsc`, `crossbeam_channel`, or `tokio::sync::mpsc`).
- Shared state with `Arc<Mutex<T>>` is a fallback for cases where messages
  feel forced.

```rust
let (tx, rx) = std::sync::mpsc::channel();

std::thread::spawn(move || {
    for event in detect_events() {
        tx.send(event).expect("receiver dropped");
    }
});

for event in rx {
    handle(event);
}
```

## Lock Discipline

- Hold a `Mutex` or `RwLock` for as little time as possible. Pull data out,
  drop the guard, then do the work.
- Never hold a lock across:
  - `await` points
  - blocking I/O
  - calls into unknown user code (callbacks, trait methods you do not own)
- A second lock taken inside a critical section is a red flag. Re-think the
  data layout before doing it.

## Thread Roles

- Separate input, processing, and output into different threads or tasks.
- Real-time callbacks (audio, input events) only do the minimum required and
  hand work off through a channel.
- A blocked UI must never block detection logic, and vice versa.

## `Send` And `Sync`

- Trust the compiler. If a type is not `Send` or not `Sync`, that is a signal
  about the design.
- `unsafe impl Send` and `unsafe impl Sync` are forbidden without an explicit
  written justification.

## Threads And Tasks

- Keep `JoinHandle` if the spawning code needs the result. Otherwise drop it
  intentionally rather than ignoring it.
- Do not call `unwrap()` on a `JoinHandle::join()` in production code; convert
  the panic into a logged error.
- Adopt async only when concurrent I/O justifies it. Do not paint a synchronous
  module with `async` for style.

## Cancellation

- A long-running thread or task takes a stop signal (`AtomicBool`, channel,
  `CancellationToken`). It does not loop forever with no exit path.
- Shut down channels and signal stop before joining threads.
