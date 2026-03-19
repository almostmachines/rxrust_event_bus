use std::convert::Infallible;

use rxrust::{Local, LocalBoxedObservableClone, LocalSubject, Observable, ObservableFactory, Observer, Subscription};

type EventBusInner<E> = LocalSubject<'static, E, Infallible>;
type EventStream<E> = LocalBoxedObservableClone<'static, E, Infallible>;

#[derive(Clone)]
pub struct EventBus<E> {
    inner: EventBusInner<E>,
}

impl<E: Clone + 'static> Default for EventBus<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Clone + 'static> EventBus<E> {
    pub fn new() -> Self {
        Self {
            inner: Local::subject::<E, Infallible>(),
        }
    }

    pub fn publish(&self, event: E) {
        let mut subject = self.inner.clone();
        subject.next(event);
    }

    pub fn events(&self) -> EventStream<E> {
        self.inner.clone().box_it_clone()
    }

    pub fn subscribe<F>(&self, handler: F) -> impl Subscription + use<E, F>
where
        F: FnMut(E) + 'static,
    {
        self.events().subscribe(handler)
    }
}
