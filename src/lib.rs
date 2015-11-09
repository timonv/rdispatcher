use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::hash::{Hash, SipHasher, Hasher};
use std::clone::Clone;

// Aliases for easier refactoring
pub type SubscribeHandle<T> = mpsc::Sender<DispatchMessage<T>>;
pub type BroadcastHandle<T> = mpsc::Receiver<DispatchMessage<T>>;

#[derive(Clone)]
pub struct DispatchMessage<T> where T: Hash + Send + Clone {
   pub dispatch_type: T,
   pub payload: String
}

pub struct Dispatcher<T> where T: Hash + Send + Clone {
    subscribers: HashMap<String, Vec<SubscribeHandle<T>>>,
    broadcasters: Vec<Arc<Mutex<BroadcastHandle<T>>>>
}

pub trait Broadcast<T: Hash + Send + Clone> {
   fn broadcast_handle(&mut self) -> BroadcastHandle<T>;
}

pub trait Subscribe<T: Hash + Send + Clone> {
   fn subscribe_handle(&self) -> SubscribeHandle<T>;
}

impl <T: 'static + Hash + Send + Clone>Dispatcher<T> {
    pub fn new() -> Dispatcher<T> {
        Dispatcher { subscribers: HashMap::new(), broadcasters: vec![] }
    }

    pub fn register_broadcaster(&mut self, broadcaster: &mut Broadcast<T>) {
       let handle = Arc::new(Mutex::new(broadcaster.broadcast_handle()));
       self.broadcasters.push(handle);
    }

    pub fn register_subscriber(&mut self, subscriber: &Subscribe<T>, dispatch_type: T) {
       let sender = subscriber.subscribe_handle();
       let type_key = type_to_string(&dispatch_type);
       let new = match self.subscribers.get_mut(&type_key) {
          Some(others) => {
             others.push(sender);
             None
          },
          None => {
             Some(vec![sender])
          }
       };
       // Improve me. Cant chain because double mut borrow not allowed
       new.and_then(|new_senders| self.subscribers.insert(type_key, new_senders));
    }

    pub fn start(&self) {
       // Assuming that broadcasters.clone() copies the vector, but increase ref count on children
       for broadcaster in self.broadcasters.clone() {
          let subscribers = self.subscribers.clone();
          thread::spawn(move || {
             loop {
                let message = broadcaster.lock().unwrap().recv().ok().expect("Couldn't receive message in broadcaster or channel hung up");
                match subscribers.get(&type_to_string(&message.dispatch_type)) {
                  Some(ref subs) => { 
                      for sub in subs.iter() { sub.send(message.clone()).unwrap(); }
                  },
                  None => ()
                }

             }
          });
       }
    }

    #[allow(dead_code)]
    fn num_broadcasters(&self) -> usize {
       self.broadcasters.len()
    }

    #[allow(dead_code)]
    fn num_subscribers(&self, dispatch_type: T) -> usize {
       match self.subscribers.get(&type_to_string(&dispatch_type)) {
          Some(subscribers) => subscribers.len(),
          None => 0
       }
    }
}

// Convert to hashable for dispatchtype?
fn type_to_string<T: Hash>(t: &T) -> String {
   let mut s = SipHasher::new();
   t.hash(&mut s);
   s.finish().to_string()
}

#[cfg(test)]
mod test {
    use std::sync::mpsc;
    use self::DispatchType::*;
    use super::*;

    #[derive(PartialEq, Clone, Hash, Debug)]
    pub enum DispatchType {
        OutgoingMessage,
        RawIncomingMessage
    }

    fn setup_dispatcher() -> Dispatcher<DispatchType> {
        Dispatcher::new()
    }

    #[test]
    fn test_register_broadcaster() {
        let mut dispatcher = setup_dispatcher();
        let mut brd = TestBroadcaster::new();
        assert_eq!(dispatcher.num_broadcasters(), 0);
        dispatcher.register_broadcaster(&mut brd);
        assert_eq!(dispatcher.num_broadcasters(), 1);
    }

    #[test]
    fn test_register_subscriber() {
        let mut dispatcher = setup_dispatcher();
        let sub = TestSubscriber::new();
        assert_eq!(dispatcher.num_subscribers(OutgoingMessage), 0);
        dispatcher.register_subscriber(&sub, OutgoingMessage);
        assert_eq!(dispatcher.num_subscribers(OutgoingMessage), 1);
    }

    #[test]
    fn test_register_multiple_subscribers() {
        let mut dispatcher = setup_dispatcher();
        let sub = TestSubscriber::new();
        let sub2 = TestSubscriber::new();

        assert_eq!(dispatcher.num_subscribers(OutgoingMessage), 0);
        dispatcher.register_subscriber(&sub, OutgoingMessage);
        dispatcher.register_subscriber(&sub2, OutgoingMessage);
        assert_eq!(dispatcher.num_subscribers(OutgoingMessage), 2);
    }

    #[test]
    fn test_broadcast_simple_message() {
        let mut dispatcher = setup_dispatcher();
        let sub = TestSubscriber::new();
        let mut brd = TestBroadcaster::new();
        dispatcher.register_broadcaster(&mut brd);
        dispatcher.register_subscriber(&sub, OutgoingMessage);

        dispatcher.start();

        brd.broadcast(OutgoingMessage, "Hello world!".to_string());
        let message = sub.receiver.recv().unwrap();
        assert_eq!(message.dispatch_type, OutgoingMessage);
        assert_eq!(message.payload, "Hello world!");
    }

    #[test]
    fn test_broadcast_multiple_to_one() {
        let mut dispatcher = setup_dispatcher();
        let sub = TestSubscriber::new();
        let mut brd = TestBroadcaster::new();
        dispatcher.register_broadcaster(&mut brd);
        dispatcher.register_subscriber(&sub, OutgoingMessage);
        dispatcher.register_subscriber(&sub, RawIncomingMessage);

        dispatcher.start();

        brd.broadcast(OutgoingMessage, "Hello world!".to_string());
        let message = sub.receiver.recv().unwrap();
        assert_eq!(message.dispatch_type, OutgoingMessage);
        assert_eq!(message.payload, "Hello world!");
        brd.broadcast(RawIncomingMessage, "Hello world!".to_string());
        let message = sub.receiver.recv().unwrap();
        assert_eq!(message.dispatch_type, RawIncomingMessage);
        assert_eq!(message.payload, "Hello world!");
    }

    // #[test]
    // TODO Test is pending, this features needs to be implemented
    // or the problem avoided
    //
    // fn test_broadcast_simple_message_with_complex_enum() {
    //     let mut dispatcher = setup_dispatcher();
    //     let sub = TestSubscriber::new();
    //     let mut brd = TestBroadcaster::new();
    //     dispatcher.register_broadcaster(&mut brd);
    //     dispatcher.register_subscriber(&sub, SomethingComplex(String::new()));

    //     dispatcher.start();

    //     brd.broadcast(SomethingComplex("abc".to_string()), "Hello world!".to_string());
    //     let message = sub.receiver.recv().unwrap();
    //     assert_eq!(message.dispatch_type, SomethingComplex("abc".to_string()));
    //     assert_eq!(message.payload, "Hello world!");
    // }

    struct TestBroadcaster {
       sender: Option<SubscribeHandle<DispatchType>>
    }

    impl TestBroadcaster {
       fn new() -> TestBroadcaster {
         TestBroadcaster { sender: None }
      }

      fn broadcast(&self, dispatch_type: DispatchType, payload: String) {
         let message = DispatchMessage { dispatch_type: dispatch_type, payload: payload };
         match self.sender {
            Some(ref s) => { s.send(message).unwrap(); },
            None => ()
         };
      }
    }

    impl Broadcast<DispatchType> for TestBroadcaster {
      fn broadcast_handle(&mut self) -> BroadcastHandle<DispatchType> {
         let (tx, rx) = mpsc::channel::<DispatchMessage<DispatchType>>();
         self.sender = Some(tx);
         rx
      }

    }

    struct TestSubscriber {
      receiver: BroadcastHandle<DispatchType>,
      sender: SubscribeHandle<DispatchType>
    }

    impl TestSubscriber {
       fn new() -> TestSubscriber {
          let(tx, rx) = mpsc::channel::<DispatchMessage<DispatchType>>();
          TestSubscriber { receiver: rx, sender: tx }
       }
    }

    impl Subscribe<DispatchType> for TestSubscriber {
       fn subscribe_handle(&self) -> SubscribeHandle<DispatchType> {
          self.sender.clone()
       }
    }
}
