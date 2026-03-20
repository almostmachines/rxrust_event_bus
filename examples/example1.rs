use rxrust::Observable;
use rxrust_event_bus::LocalEventBus;

#[derive(Clone, Debug)]
enum ChatEvent {
    Message { user: String, text: String },
    UserJoined { user: String },
}

#[derive(Debug)]
enum ChatError {
    EmptyMessage,
    EmptyUsername,
}

fn validate(event: ChatEvent) -> Result<ChatEvent, ChatError> {
    match &event {
        ChatEvent::Message { user, text } => {
            if user.is_empty() {
                return Err(ChatError::EmptyUsername);
            }
            if text.is_empty() {
                return Err(ChatError::EmptyMessage);
            }
        }
        ChatEvent::UserJoined { user } => {
            if user.is_empty() {
                return Err(ChatError::EmptyUsername);
            }
        }
    }
    Ok(event)
}

fn main() {
    let bus = LocalEventBus::new(validate);

    // Subscribe to all events
    bus.subscribe(|event| {
        println!("  [log] {:?}", event);
    });

    // Subscribe to the event stream with filtering (only messages)
    bus.events()
        .filter(|e| matches!(e, ChatEvent::Message { .. }))
        .subscribe(|event| {
            if let ChatEvent::Message { user, text } = event {
                println!("  [chat] {user}: {text}");
            }
        });

    // Publish some events
    println!("Publishing UserJoined:");
    bus.publish(ChatEvent::UserJoined {
        user: "Alice".into(),
    })
    .unwrap();

    println!("Publishing Message:");
    bus.publish(ChatEvent::Message {
        user: "Alice".into(),
        text: "Hello, world!".into(),
    })
    .unwrap();

    // Validation rejects invalid events
    println!("Publishing invalid (empty message):");
    if let Err(e) = bus.publish(ChatEvent::Message {
        user: "Alice".into(),
        text: "".into(),
    }) {
        println!("  Rejected: {:?}", e);
    }
}
