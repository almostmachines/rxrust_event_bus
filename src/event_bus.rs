use std::{convert::Infallible, rc::Rc};

use rxrust::{
    Local, LocalBoxedObservableClone, LocalSubject, Observable, ObservableFactory, Observer,
    Subscription,
};

type EventBusInner<E> = LocalSubject<'static, E, Infallible>;
type EventStream<E> = LocalBoxedObservableClone<'static, E, Infallible>;
type EventValidator<E, V> = Rc<dyn Fn(E) -> Result<E, V> + 'static>;

#[derive(Clone)]
pub struct EventBus<E, V> {
    inner: EventBusInner<E>,
    validate: EventValidator<E, V>,
}

impl<E: Clone + 'static, V> EventBus<E, V> {
    pub fn new<F>(validator: F) -> Self
    where
        F: Fn(E) -> Result<E, V> + 'static,
    {
        Self {
            inner: Local::subject::<E, Infallible>(),
            validate: Rc::new(validator),
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
        S: FnMut(E) + 'static,
    {
        self.events().subscribe(handler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Clone, Debug, PartialEq)]
    enum Event {
        InfoEvent {
            published_on: u32,
            data: String,
        },
        ErrorEvent {
            published_on: u32,
            data: String,
        },
    }

    #[derive(Clone, Debug, PartialEq)]
    enum EventError {
        EventAlreadyPublished,
        EmptyData,
    }

    fn validate_event(event: Event) -> Result<Event, EventError> {
        let (published_on, data) = match &event {
            Event::InfoEvent { published_on, data }
            | Event::ErrorEvent { published_on, data } => (published_on, data),
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
        let event_bus = EventBus::new(validate_event);
        let event = Event::InfoEvent { published_on: 15, data: String::from("event") };
        let result = event_bus.publish(event);

        assert_eq!(result, Err(EventError::EventAlreadyPublished));
    }

    #[test]
    fn test_invalid_event_not_published() {
        let event_bus = EventBus::new(validate_event);
        let event = Event::InfoEvent { published_on: 15, data: String::from("event") };
        let events: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));
        let events_clone = events.clone();

        event_bus.subscribe(move |e| events_clone.borrow_mut().push(e));

        let _ = event_bus.publish(event);

        assert_eq!(events.borrow().len(), 0);
    }

    #[test]
    fn test_valid_event_returns_ok() {
        let event_bus = EventBus::new(validate_event);
        let event = Event::InfoEvent { published_on: 0, data: String::from("event") };
        let event_clone = event.clone();
        let result = event_bus.publish(event);

        assert_eq!(result, Ok(event_clone));
    }

    #[test]
    fn test_valid_event_is_received_by_subscriber() {
        let event_bus = EventBus::new(validate_event);
        let event = Event::InfoEvent { published_on: 0, data: String::from("event") };
        let events: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));
        let events_clone = events.clone();

        event_bus.subscribe(move |e| events_clone.borrow_mut().push(e));

        let _ = event_bus.publish(event.clone());

        assert_eq!(events.borrow().len(), 1);
        assert_eq!(events.borrow()[0], event)
    }

    #[test]
    fn test_multiple_events_received_by_subscriber() {
        use std::cell::RefCell;

        let event_bus = EventBus::new(validate_event);
        let event1 = Event::InfoEvent { published_on: 0, data: String::from("event 1") };
        let event1_clone = event1.clone();
        let event2 = Event::ErrorEvent { published_on: 0, data: String::from("event 2") };
        let event2_clone = event2.clone();
        let events: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));
        let events_clone = events.clone();

        event_bus.subscribe(move |e| events_clone.borrow_mut().push(e));

        let _ = event_bus.publish(event1);
        let _ = event_bus.publish(event2);

        assert_eq!(events.borrow().len(), 2);
        assert_eq!(events.borrow()[0], event1_clone);
        assert_eq!(events.borrow()[1], event2_clone);
    }

    #[test]
    fn test_valid_event_is_published_to_events_stream() {
        use std::cell::RefCell;

        let event_bus = EventBus::new(validate_event);
        let event = Event::InfoEvent { published_on: 0, data: String::from("event") };
        let event_clone = event.clone();
        let events: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));
        let events_clone = events.clone();

        event_bus.events().subscribe(move |e| events_clone.borrow_mut().push(e));

        let _ = event_bus.publish(event);

        assert_eq!(events.borrow().len(), 1);
        assert_eq!(events.borrow()[0], event_clone);
    }

    #[test]
    fn test_multiple_events_published_to_events_stream() {
        use std::cell::RefCell;

        let event_bus = EventBus::new(validate_event);
        let event1 = Event::InfoEvent { published_on: 0, data: String::from("event 1") };
        let event2 = Event::ErrorEvent { published_on: 0, data: String::from("event 2") };
        let event1_clone = event1.clone();
        let event2_clone = event2.clone();
        let events: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));
        let events_clone = events.clone();

        event_bus.events().subscribe(move |e| events_clone.borrow_mut().push(e));

        let _ = event_bus.publish(event1);
        let _ = event_bus.publish(event2);

        assert_eq!(events.borrow().len(), 2);
        assert_eq!(events.borrow()[0], event1_clone);
        assert_eq!(events.borrow()[1], event2_clone);
    }

    #[test]
    fn test_event_is_received_by_multiple_subscribers() {
        let event_bus = EventBus::new(validate_event);
        let event = Event::InfoEvent { published_on: 0, data: String::from("event") };
        let events1: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));
        let events2: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));
        let events1_clone = events1.clone();
        let events2_clone = events2.clone();

        event_bus.subscribe(move |e| events1_clone.borrow_mut().push(e));
        event_bus.subscribe(move |e| events2_clone.borrow_mut().push(e));

        let _ = event_bus.publish(event.clone());

        assert_eq!(events1.borrow().len(), 1);
        assert_eq!(events1.borrow()[0], event);
        assert_eq!(events2.borrow().len(), 1);
        assert_eq!(events2.borrow()[0], event);
    }

    #[test]
    fn test_events_published_prior_to_subscription_not_received() {
        let event_bus = EventBus::new(validate_event);
        let event = Event::InfoEvent { published_on: 0, data: String::from("event") };
        let events: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));
        let events_clone = events.clone();
        let _ = event_bus.publish(event.clone());

        event_bus.subscribe(move |e| events_clone.borrow_mut().push(e));

        assert_eq!(events.borrow().len(), 0);
    }
}
