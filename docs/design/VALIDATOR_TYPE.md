# LocalEventBus Validator Clone Discussion

## Problem

`LocalEventBus` currently derives `Clone`, but one of its fields is:

```rust
type EventValidator<E, V> = Box<dyn Fn(E) -> Result<E, V>>;
```

`Box<dyn Fn(...)>` is a boxed trait object. Trait objects do not implement
`Clone` unless the trait explicitly supports cloning, so `#[derive(Clone)]`
fails.

The real design question is not just "how do we make the compiler happy?"
It is "what cloning an `LocalEventBus` should mean for users of this library?"

## What We Want To Optimize For

For this library, the main user-facing concerns are:

- Can callers pass ordinary closures to `LocalEventBus::new`?
- Can the bus itself still be cloned ergonomically?
- Does the API stay simple in function signatures and struct fields?
- Does the design match the current `rxrust::Local` single-threaded model?
- Are we adding complexity or dependencies that the project does not need?

## Option 1: Remove `Clone` From `LocalEventBus`

### Shape

Keep the validator as:

```rust
type EventValidator<E, V> = Box<dyn Fn(E) -> Result<E, V>>;
```

but remove `#[derive(Clone)]` from `LocalEventBus`.

### What This Means For Library Users

Users can still create the bus with a capturing closure and use it normally.
The difference is that the bus becomes a single-owner value unless the caller
wraps it themselves in `Rc<LocalEventBus<...>>` or `Arc<LocalEventBus<...>>`.

### Pros

- Smallest code change.
- Keeps the current boxed-closure API.
- Supports stateful validators without any extra machinery.
- No extra dependency and no public API complexity.

### Cons

- Cloning the bus is no longer part of the library API.
- Callers must introduce their own `Rc` or `Arc` if they want shared access.
- That pushes an ownership decision onto every consumer.
- If `Clone` was already part of the intended public contract, removing it is a
  meaningful API downgrade.

### Best Fit

This is a good option if the bus is meant to have one owner, or if cloning was
just a convenience and not an important part of the design.

## Option 2: Store The Validator In `Rc` Or `Arc`

### Shape

Single-threaded version:

```rust
use std::rc::Rc;

type EventValidator<E, V> = Rc<dyn Fn(E) -> Result<E, V>>;
```

Typical constructor:

```rust
pub fn new<F>(validator: F) -> Self
where
    F: Fn(E) -> Result<E, V> + 'static,
{
    Self {
        inner: Local::subject::<E, Infallible>(),
        validate: Rc::new(validator),
    }
}
```

Thread-safe variant:

```rust
use std::sync::Arc;

type EventValidator<E, V> = Arc<dyn Fn(E) -> Result<E, V> + Send + Sync>;
```

### What This Means For Library Users

Users still pass ordinary closures to `LocalEventBus::new`. `LocalEventBus` stays
cloneable, and every clone shares the same validator.

This is usually the most ergonomic library API because callers do not need to
know or care how the validator is stored internally.

### Pros

- Keeps `LocalEventBus: Clone`.
- Keeps constructor usage simple for callers.
- Works well with capturing closures.
- No extra crate is needed.
- `Rc` matches the current `LocalSubject` design, which is already local and
  not thread-safe.

### Cons

- Cloned buses share the same validator instance, they do not get independent
  validator copies.
- If the closure captures mutable shared state, all clones observe that shared
  state.
- `Rc` is not `Send` or `Sync`, so this choice bakes in single-threaded usage.
- `Arc` can solve the thread-sharing part, but then the validator usually also
  needs `Send + Sync`, which makes some closures harder to write.

### Best Fit

This is the best fit if the library wants a simple and ergonomic API, expects
the bus to be cloned, and is comfortable with shared validator behavior.

## Option 3: Make The Validator A Generic Type Parameter

### Shape

```rust
pub struct LocalEventBus<E, V, F>
where
    F: Fn(E) -> Result<E, V>,
{
    inner: LocalEventBusInner<E>,
    validate: F,
}
```

Then `Clone` can be derived or implemented only when `F: Clone`.

### What This Means For Library Users

For straightforward construction, this can still feel nice because type
inference usually figures out `F`:

```rust
let bus = LocalEventBus::new(|event| Ok(event));
```

The tradeoff appears when users need to mention the type in public APIs, struct
fields, trait impls, or collections. Then the validator type becomes part of
the bus type, which can make signatures much more verbose.

### Pros

- No heap allocation for the validator.
- No dynamic dispatch on validator calls.
- Very flexible for optimization-minded users.
- `Clone` works naturally when the concrete validator type is cloneable.

### Cons

- The public type becomes more complex: `LocalEventBus<E, V, F>`.
- Callers may need extra generics in their own code just to store or pass a
  bus around.
- Different validators produce different bus types, which makes heterogeneous
  storage harder.
- The library may produce more monomorphized code, which can increase compile
  time and binary size.
- Closures are only cloneable when everything they capture is also cloneable.

### Best Fit

This is a strong option if performance and static typing matter more than API
simplicity, or if the library is intentionally low-level.

## Option 4: Use A Plain Function Pointer

### Shape

```rust
type EventValidator<E, V> = fn(E) -> Result<E, V>;
```

### What This Means For Library Users

Users can pass named functions and non-capturing closures that coerce to a
function pointer. They cannot pass closures that capture configuration or
shared state.

### Pros

- Very simple type.
- Trivially `Copy` and `Clone`.
- No heap allocation and no reference counting.
- Easy to understand and easy to document.

### Cons

- Much less flexible than the other options.
- Stateful validation logic becomes awkward or impossible.
- Users may have to move configuration into globals or redesign their code
  around the API.
- It is a poor fit if validators are expected to depend on runtime state.

### Best Fit

This works well only if the library wants validators to be intentionally simple
and stateless.

## Option 5: Keep `Box<dyn Fn>` And Add Clone Support For Trait Objects

### Shape

This is usually done with a helper crate such as `dyn-clone`, or with a custom
trait-object cloning pattern.

Conceptually, the validator becomes "a trait object that also knows how to
clone itself."

### What This Means For Library Users

From the user's point of view, this preserves the boxed trait-object style and
can keep the public API looking compact. Callers can keep passing closures, but
cloning only works when the concrete closure type is actually cloneable.

### Pros

- Preserves a compact trait-object based API.
- Keeps `LocalEventBus: Clone` available.
- Allows each bus clone to own a cloned validator value rather than just
  another shared pointer to the same validator handle.

### Cons

- More implementation machinery than the project likely needs.
- Usually adds a dependency or a custom cloning trait.
- Error messages get more advanced because trait-object cloning is a more
  specialized Rust pattern.
- It still does not make every closure cloneable; captured state must support
  cloning too.

### Best Fit

This is most useful when we specifically want boxed trait objects with
clone-by-value semantics and are willing to pay for the extra complexity.

## Recommendation

For this project as it exists today, `Rc<dyn Fn(E) -> Result<E, V>>` is the
best default.

Why:

- It keeps the library easy to use.
- It preserves `LocalEventBus: Clone`.
- It lets users pass ordinary capturing closures.
- It matches the current `rxrust::Local` single-threaded design.
- It avoids adding a dependency purely to clone a validator.

If the library later grows a cross-thread event bus, the same design can be
revisited with `Arc<dyn Fn(...) + Send + Sync>`.

If the library decides that cloning the bus is not important, then removing
`Clone` is the simplest and most honest API.
