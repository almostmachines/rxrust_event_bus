# Subscription Lifetime Note

This project hit a Rust 2024 lifetime error around `EventBus::subscribe` in
`src/main.rs`.

## The symptom

The compiler reported that `bus` did not live long enough when this code in
`StatusPanel::new` tried to keep the subscription alive:

```rust
let subscription = bus
    .subscribe(move |event| match event {
        AppEvent::SaveRequested => {
            *status_text_for_handler.borrow_mut() = "Saving...".to_string();
        }
        AppEvent::SaveFinished { ok } => {
            *status_text_for_handler.borrow_mut() =
                if ok { "Save succeeded" } else { "Save failed" }.to_string();
        }
    })
    .into_boxed()
    .unsubscribe_when_dropped();
```

## What was actually happening

The problem was not the closure, and it was not `LocalSubject<'static, ...>`.
The real issue was the wrapper method signature:

```rust
fn subscribe<F>(&self, handler: F) -> impl Subscription
where
    F: FnMut(E) + 'static,
```

In Rust 2024, return-position `impl Trait` captures more lifetimes by default.
Because this method takes `&self`, the hidden type behind `impl Subscription`
was treated as if it might capture the lifetime of that borrow.

That made the returned value look tied to `bus`, even though the concrete
subscription produced by `rxrust` is an owned handle and does not actually
borrow the subject.

## Why that turned into an error

The subscription was immediately boxed with `into_boxed()`. `rxrust`
intentionally requires boxed subscriptions to be `'static`, because they are
meant to be movable, storable control handles.

So the compiler saw two conflicting requirements:

- the returned `impl Subscription` might borrow `&self`
- `into_boxed()` requires the subscription to be `'static`

Those cannot both be true, so Rust rejected the code and pointed at `bus`.

## The implemented fix

We made the capture set explicit:

```rust
fn subscribe<F>(&self, handler: F) -> impl Subscription + use<E, F>
where
    F: FnMut(E) + 'static,
{
    self.inner.clone().subscribe(handler)
}
```

`use<E, F>` tells Rust that the opaque return type may depend on the event type
and the handler type, but should not capture the lifetime of `&self`.

That matches the real behavior of the code:

- the handler type matters
- the returned subscription does not borrow `EventBus`

After this change, `cargo check` passes.

## Why this fix is the best fit here

- it is the smallest possible change
- it preserves the existing API shape
- it documents the intended lifetime behavior directly in the signature
- it matches Rust 2024's precise capture model instead of working around it

## Other valid options

- return `BoxedSubscription` directly instead of `impl Subscription`
- take `self` by value instead of `&self`
- store the concrete subscription type instead of boxing it
- switch editions, though that only hides the underlying capture issue

## Takeaway

When a Rust 2024 method returns `impl Trait` from a `&self` receiver, the
opaque return type may capture the receiver lifetime unless you say otherwise.
If the returned value is really owned, use precise capture syntax such as
`use<E, F>` to prevent accidental over-capture.
