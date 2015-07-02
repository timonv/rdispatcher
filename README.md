# Rust Dispatcher

The Dispatcher allows you to send messages from multiple broadcasters to multiple
receivers. When a lot of messages get passed around a threaded application, centralizing
where messages flow through can create much wanted oversight.

The types of messages that get passed around are strongly typed, required by
you to setup, and the messages themselves are strings.

# Install

Add to your `Cargo.toml`

```
rdispatcher = "*"
```

And run `cargo install`

# Usage

A simple example:

```rust
  let mut dispatcher = Dispatcher::new();
  let sub = TestSubscriber::new();
  let mut brd = TestBroadcaster::new();
  dispatcher.register_broadcaster(&mut brd);
  dispatcher.register_subscriber(&sub, OutgoingMessage);

  dispatcher.start();

  brd.broadcast(OutgoingMessage, "Hello world!".to_string());
  let message = sub.receiver.recv().unwrap();
  assert_eq!(message.dispatch_type, OutgoingMessage);
  assert_eq!(message.payload, "Hello world!");
```

For several full examples, checkout the tests in lib.rs

# Caveats

* ~~Currently the DispatchType (I prefer using an enum for it) is cast to a string
using Debug. This is legacy and should just use Hash. If you have a custom Debug
for your your enum, you might get unexpected results.~~ Resolved
* Complex enums (i.e. SomethingComplex(String)) currently do not work as the dispatch
type and will yield weird results.
