use std::{convert::Infallible, sync::Arc};

use rxrust::{
    Observable, ObservableFactory, Observer, Shared, SharedBoxedObservableClone, SharedSubject,
    Subscription,
};

type SharedEventBusInner<E> = SharedSubject<'static, E, Infallible>;
type EventStream<E> = SharedBoxedObservableClone<'static, E, Infallible>;
type EventValidator<E, V> = Arc<dyn Fn(E) -> Result<E, V> + Send + Sync + 'static>;

#[derive(Clone)]
pub struct SharedEventBus<E, V> {
    inner: SharedEventBusInner<E>,
    validate: EventValidator<E, V>,
}

impl<E: Clone + Send + 'static, V> SharedEventBus<E, V> {
    pub fn new<F>(validator: F) -> Self
    where
        F: Fn(E) -> Result<E, V> + Send + Sync + 'static,
    {
        Self {
            inner: Shared::subject::<E, Infallible>(),
            validate: Arc::new(validator),
        }
    }

    pub fn publish(&self, event: E) -> Result<E, V> {
        let mut subject = self.inner.clone();
        let validated = (self.validate)(event);

        if let Ok(evt) = &validated {
            subject.next(evt.clone());
        }

        validated
    }

    pub fn events(&self) -> EventStream<E> {
        self.inner.clone().box_it_clone()
    }

    pub fn subscribe<S>(&self, handler: S) -> impl Subscription + use<E, V, S>
    where
        S: FnMut(E) + Send + 'static,
    {
        self.events().subscribe(handler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier, Mutex};

    #[derive(Clone, Debug, PartialEq)]
    enum Event {
        InfoEvent { published_on: u32, data: String },
        ErrorEvent { published_on: u32, data: String },
    }

    #[derive(Clone, Debug, PartialEq)]
    enum EventError {
        EventAlreadyPublished,
        EmptyData,
    }

    fn validate_event(event: Event) -> Result<Event, EventError> {
        let (published_on, data) = match &event {
            Event::InfoEvent { published_on, data } | Event::ErrorEvent { published_on, data } => {
                (published_on, data)
            }
        };

        if *published_on > 0 {
            return Err(EventError::EventAlreadyPublished);
        }

        if data.is_empty() {
            return Err(EventError::EmptyData);
        }

        Ok(event)
    }

    #[test]
    fn test_invalid_event_returns_err() {
        let event_bus = SharedEventBus::new(validate_event);
        let event = Event::InfoEvent {
            published_on: 15,
            data: String::from("event"),
        };
        let result = event_bus.publish(event);

        assert_eq!(result, Err(EventError::EventAlreadyPublished));
    }

    #[test]
    fn test_invalid_event_not_published() {
        let event_bus = SharedEventBus::new(validate_event);
        let event = Event::InfoEvent {
            published_on: 15,
            data: String::from("event"),
        };
        let events: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        event_bus.subscribe(move |e| events_clone.lock().unwrap().push(e));

        let _ = event_bus.publish(event);

        assert_eq!(events.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_valid_event_returns_ok() {
        let event_bus = SharedEventBus::new(validate_event);
        let event = Event::InfoEvent {
            published_on: 0,
            data: String::from("event"),
        };
        let event_clone = event.clone();
        let result = event_bus.publish(event);

        assert_eq!(result, Ok(event_clone));
    }

    #[test]
    fn test_valid_event_is_received_by_subscriber() {
        let event_bus = SharedEventBus::new(validate_event);
        let event = Event::InfoEvent {
            published_on: 0,
            data: String::from("event"),
        };
        let events: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        event_bus.subscribe(move |e| events_clone.lock().unwrap().push(e));

        let _ = event_bus.publish(event.clone());

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], event);
    }

    #[test]
    fn test_multiple_events_received_by_subscriber() {
        let event_bus = SharedEventBus::new(validate_event);
        let event1 = Event::InfoEvent {
            published_on: 0,
            data: String::from("event 1"),
        };
        let event1_clone = event1.clone();
        let event2 = Event::ErrorEvent {
            published_on: 0,
            data: String::from("event 2"),
        };
        let event2_clone = event2.clone();
        let events: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        event_bus.subscribe(move |e| events_clone.lock().unwrap().push(e));

        let _ = event_bus.publish(event1);
        let _ = event_bus.publish(event2);

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], event1_clone);
        assert_eq!(events[1], event2_clone);
    }

    #[test]
    fn test_valid_event_is_published_to_events_stream() {
        let event_bus = SharedEventBus::new(validate_event);
        let event = Event::InfoEvent {
            published_on: 0,
            data: String::from("event"),
        };
        let event_clone = event.clone();
        let events: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        event_bus
            .events()
            .subscribe(move |e| events_clone.lock().unwrap().push(e));

        let _ = event_bus.publish(event);

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], event_clone);
    }

    #[test]
    fn test_multiple_events_published_to_events_stream() {
        let event_bus = SharedEventBus::new(validate_event);
        let event1 = Event::InfoEvent {
            published_on: 0,
            data: String::from("event 1"),
        };
        let event2 = Event::ErrorEvent {
            published_on: 0,
            data: String::from("event 2"),
        };
        let event1_clone = event1.clone();
        let event2_clone = event2.clone();
        let events: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        event_bus
            .events()
            .subscribe(move |e| events_clone.lock().unwrap().push(e));

        let _ = event_bus.publish(event1);
        let _ = event_bus.publish(event2);

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], event1_clone);
        assert_eq!(events[1], event2_clone);
    }

    #[test]
    fn test_event_is_received_by_multiple_subscribers() {
        let event_bus = SharedEventBus::new(validate_event);
        let event = Event::InfoEvent {
            published_on: 0,
            data: String::from("event"),
        };
        let events1: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));
        let events2: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));
        let events1_clone = events1.clone();
        let events2_clone = events2.clone();

        event_bus.subscribe(move |e| events1_clone.lock().unwrap().push(e));
        event_bus.subscribe(move |e| events2_clone.lock().unwrap().push(e));

        let _ = event_bus.publish(event.clone());

        let events1 = events1.lock().unwrap();
        let events2 = events2.lock().unwrap();
        assert_eq!(events1.len(), 1);
        assert_eq!(events1[0], event);
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0], event);
    }

    #[test]
    fn test_events_published_prior_to_subscription_not_received() {
        let event_bus = SharedEventBus::new(validate_event);
        let event = Event::InfoEvent {
            published_on: 0,
            data: String::from("event"),
        };
        let events: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let _ = event_bus.publish(event.clone());

        event_bus.subscribe(move |e| events_clone.lock().unwrap().push(e));

        assert_eq!(events.lock().unwrap().len(), 0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_event_can_be_published_from_another_thread() {
        let event_bus = SharedEventBus::new(validate_event);
        let event = Event::InfoEvent {
            published_on: 0,
            data: String::from("event"),
        };
        let events: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let _subscription = event_bus.subscribe(move |e| events_clone.lock().unwrap().push(e));

        let event_bus_clone = event_bus.clone();
        let event_clone = event.clone();
        std::thread::spawn(move || {
            let _ = event_bus_clone.publish(event_clone);
        })
        .join()
        .unwrap();

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], event);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_event_can_be_subscribed_to_from_another_thread() {
        let event_bus = SharedEventBus::new(validate_event);
        let event = Event::InfoEvent {
            published_on: 0,
            data: String::from("event"),
        };
        let events: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));
        let ready = Arc::new(Barrier::new(2));
        let done = Arc::new(Barrier::new(2));

        let event_bus_clone = event_bus.clone();
        let events_clone = events.clone();
        let ready_clone = ready.clone();
        let done_clone = done.clone();

        let handle = std::thread::spawn(move || {
            let _subscription =
                event_bus_clone.subscribe(move |e| events_clone.lock().unwrap().push(e));
            ready_clone.wait();
            done_clone.wait();
        });

        ready.wait();
        let _ = event_bus.publish(event.clone());
        done.wait();
        handle.join().unwrap();

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], event);
    }
}
