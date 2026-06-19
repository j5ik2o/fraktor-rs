use alloc::{string::String, vec::Vec};
use core::cmp::Ordering;

use crate::{
  actor::{
    Pid,
    actor_ref::{ActorRef, NullSender},
  },
  event::bus::{
    ActorClassifier, ActorEventBus, EventBus, LookupClassification, ManagedActorClassification, PredicateClassifier,
    ScanningClassification, SubchannelClassification,
  },
};

#[derive(Clone)]
struct Envelope {
  topic:   &'static str,
  payload: u32,
}

#[derive(Default)]
struct LookupBus {
  subscriptions: Vec<(&'static str, u32)>,
  delivered:     Vec<(u32, u32)>,
}

impl EventBus for LookupBus {
  type Classifier = &'static str;
  type Event = Envelope;
  type Subscriber = u32;

  fn subscribe(&mut self, subscriber: Self::Subscriber, to: Self::Classifier) -> bool {
    if self.subscriptions.iter().any(|(classifier, current)| *classifier == to && *current == subscriber) {
      return false;
    }
    self.subscriptions.push((to, subscriber));
    true
  }

  fn unsubscribe(&mut self, subscriber: &Self::Subscriber, from: &Self::Classifier) -> bool {
    let before = self.subscriptions.len();
    self.subscriptions.retain(|(classifier, current)| classifier != from || current != subscriber);
    self.subscriptions.len() != before
  }

  fn unsubscribe_all(&mut self, subscriber: &Self::Subscriber) {
    self.subscriptions.retain(|(_, current)| current != subscriber);
  }

  fn publish(&mut self, event: Self::Event) {
    let classifier = <Self as LookupClassification>::classify(self, &event);
    let subscribers = self
      .subscriptions
      .iter()
      .filter(|(current, _)| *current == classifier)
      .map(|(_, subscriber)| *subscriber)
      .collect::<Vec<_>>();
    for subscriber in subscribers {
      <Self as LookupClassification>::publish_to(self, &event, &subscriber);
    }
  }
}

impl LookupClassification for LookupBus {
  fn map_size(&self) -> usize {
    128
  }

  fn compare_subscribers(&self, left: &Self::Subscriber, right: &Self::Subscriber) -> Ordering {
    left.cmp(right)
  }

  fn classify(&self, event: &Self::Event) -> Self::Classifier {
    event.topic
  }

  fn publish_to(&mut self, event: &Self::Event, subscriber: &Self::Subscriber) {
    self.delivered.push((*subscriber, event.payload));
  }
}

#[test]
fn lookup_classification_dispatches_by_equal_classifier() {
  let mut bus = LookupBus::default();

  assert!(bus.subscribe(10, "greetings"));
  assert!(!bus.subscribe(10, "greetings"));
  bus.publish(Envelope { topic: "time", payload: 1 });
  bus.publish(Envelope { topic: "greetings", payload: 2 });

  assert_eq!(bus.map_size(), 128);
  assert_eq!(bus.compare_subscribers(&1, &2), Ordering::Less);
  assert_eq!(bus.delivered, [(10, 2)]);
  assert!(bus.unsubscribe(&10, &"greetings"));
  bus.publish(Envelope { topic: "greetings", payload: 3 });
  assert_eq!(bus.delivered, [(10, 2)]);
}

#[derive(Default)]
struct SubchannelBus {
  subscriptions: Vec<(&'static str, u32)>,
  delivered:     Vec<(u32, u32)>,
}

impl EventBus for SubchannelBus {
  type Classifier = &'static str;
  type Event = Envelope;
  type Subscriber = u32;

  fn subscribe(&mut self, subscriber: Self::Subscriber, to: Self::Classifier) -> bool {
    if self.subscriptions.iter().any(|(classifier, current)| *classifier == to && *current == subscriber) {
      return false;
    }
    self.subscriptions.push((to, subscriber));
    true
  }

  fn unsubscribe(&mut self, subscriber: &Self::Subscriber, from: &Self::Classifier) -> bool {
    let before = self.subscriptions.len();
    self.subscriptions.retain(|(classifier, current)| classifier != from || current != subscriber);
    self.subscriptions.len() != before
  }

  fn unsubscribe_all(&mut self, subscriber: &Self::Subscriber) {
    self.subscriptions.retain(|(_, current)| current != subscriber);
  }

  fn publish(&mut self, event: Self::Event) {
    let classifier = <Self as SubchannelClassification>::classify(self, &event);
    let subscribers = self
      .subscriptions
      .iter()
      .filter(|(current, _)| {
        <Self as SubchannelClassification>::is_same_classifier(self, &classifier, current)
          || <Self as SubchannelClassification>::is_subclass(self, &classifier, current)
      })
      .map(|(_, subscriber)| *subscriber)
      .collect::<Vec<_>>();
    for subscriber in subscribers {
      <Self as SubchannelClassification>::publish_to(self, &event, &subscriber);
    }
  }
}

impl SubchannelClassification for SubchannelBus {
  fn is_same_classifier(&self, left: &Self::Classifier, right: &Self::Classifier) -> bool {
    left == right
  }

  fn is_subclass(&self, child: &Self::Classifier, parent: &Self::Classifier) -> bool {
    child.starts_with(*parent)
  }

  fn classify(&self, event: &Self::Event) -> Self::Classifier {
    event.topic
  }

  fn publish_to(&mut self, event: &Self::Event, subscriber: &Self::Subscriber) {
    self.delivered.push((*subscriber, event.payload));
  }
}

#[test]
fn subchannel_classification_dispatches_to_parent_subscribers() {
  let mut bus = SubchannelBus::default();

  assert!(bus.subscribe(7, "abc"));
  bus.publish(Envelope { topic: "xyzabc", payload: 1 });
  bus.publish(Envelope { topic: "abc", payload: 2 });
  bus.publish(Envelope { topic: "abcdef", payload: 3 });

  assert_eq!(bus.delivered, [(7, 2), (7, 3)]);
}

#[derive(Default)]
struct ScanningBus {
  subscriptions: Vec<(usize, u32)>,
  delivered:     Vec<(u32, String)>,
}

impl EventBus for ScanningBus {
  type Classifier = usize;
  type Event = String;
  type Subscriber = u32;

  fn subscribe(&mut self, subscriber: Self::Subscriber, to: Self::Classifier) -> bool {
    if self.subscriptions.iter().any(|(classifier, current)| *classifier == to && *current == subscriber) {
      return false;
    }
    self.subscriptions.push((to, subscriber));
    true
  }

  fn unsubscribe(&mut self, subscriber: &Self::Subscriber, from: &Self::Classifier) -> bool {
    let before = self.subscriptions.len();
    self.subscriptions.retain(|(classifier, current)| classifier != from || current != subscriber);
    self.subscriptions.len() != before
  }

  fn unsubscribe_all(&mut self, subscriber: &Self::Subscriber) {
    self.subscriptions.retain(|(_, current)| current != subscriber);
  }

  fn publish(&mut self, event: Self::Event) {
    let subscribers = self
      .subscriptions
      .iter()
      .filter(|(classifier, _)| <Self as ScanningClassification>::matches(self, classifier, &event))
      .map(|(_, subscriber)| *subscriber)
      .collect::<Vec<_>>();
    for subscriber in subscribers {
      <Self as ScanningClassification>::publish_to(self, &event, &subscriber);
    }
  }
}

impl ScanningClassification for ScanningBus {
  fn compare_classifiers(&self, left: &Self::Classifier, right: &Self::Classifier) -> Ordering {
    left.cmp(right)
  }

  fn compare_subscribers(&self, left: &Self::Subscriber, right: &Self::Subscriber) -> Ordering {
    left.cmp(right)
  }

  fn matches(&self, classifier: &Self::Classifier, event: &Self::Event) -> bool {
    event.len() <= *classifier
  }

  fn publish_to(&mut self, event: &Self::Event, subscriber: &Self::Subscriber) {
    self.delivered.push((*subscriber, event.clone()));
  }
}

#[test]
fn scanning_classification_dispatches_by_match_predicate() {
  let mut bus = ScanningBus::default();

  assert!(bus.subscribe(3, 3));
  bus.publish(String::from("abcd"));
  bus.publish(String::from("ab"));
  bus.publish(String::from("abc"));

  assert_eq!(bus.compare_classifiers(&1, &2), Ordering::Less);
  assert_eq!(bus.compare_subscribers(&3, &4), Ordering::Less);
  assert_eq!(bus.delivered, [(3, String::from("ab")), (3, String::from("abc"))]);
}

struct PredicateBus;

impl EventBus for PredicateBus {
  type Classifier = fn(&u32) -> bool;
  type Event = u32;
  type Subscriber = u32;

  fn subscribe(&mut self, _subscriber: Self::Subscriber, _to: Self::Classifier) -> bool {
    true
  }

  fn unsubscribe(&mut self, _subscriber: &Self::Subscriber, _from: &Self::Classifier) -> bool {
    true
  }

  fn unsubscribe_all(&mut self, _subscriber: &Self::Subscriber) {}

  fn publish(&mut self, _event: Self::Event) {}
}

#[test]
fn predicate_classifier_exposes_predicate_contract() {
  fn is_even(value: &u32) -> bool {
    value % 2 == 0
  }
  let predicate: fn(&u32) -> bool = is_even;

  assert!(<PredicateBus as PredicateClassifier>::matches_predicate(&predicate, &4));
  assert!(!<PredicateBus as PredicateClassifier>::matches_predicate(&predicate, &5));
}

#[derive(Default)]
struct ActorBus {
  associations: Vec<(ActorRef, ActorRef)>,
}

impl EventBus for ActorBus {
  type Classifier = ActorRef;
  type Event = ();
  type Subscriber = ActorRef;

  fn subscribe(&mut self, subscriber: Self::Subscriber, to: Self::Classifier) -> bool {
    <Self as ManagedActorClassification>::associate(self, to, subscriber)
  }

  fn unsubscribe(&mut self, subscriber: &Self::Subscriber, from: &Self::Classifier) -> bool {
    <Self as ManagedActorClassification>::dissociate_pair(self, from, subscriber)
  }

  fn unsubscribe_all(&mut self, subscriber: &Self::Subscriber) {
    <Self as ManagedActorClassification>::dissociate(self, subscriber);
  }

  fn publish(&mut self, _event: Self::Event) {}
}

impl ManagedActorClassification for ActorBus {
  fn map_size(&self) -> usize {
    128
  }

  fn classify(&self, _event: &Self::Event) -> ActorRef {
    actor_ref(0)
  }

  fn associate(&mut self, monitored: ActorRef, monitor: ActorRef) -> bool {
    if self
      .associations
      .iter()
      .any(|(current_monitored, current_monitor)| current_monitored == &monitored && current_monitor == &monitor)
    {
      return false;
    }
    self.associations.push((monitored, monitor));
    true
  }

  fn dissociate(&mut self, actor: &ActorRef) {
    self.associations.retain(|(monitored, monitor)| monitored != actor && monitor != actor);
  }

  fn dissociate_pair(&mut self, monitored: &ActorRef, monitor: &ActorRef) -> bool {
    let before = self.associations.len();
    self
      .associations
      .retain(|(current_monitored, current_monitor)| current_monitored != monitored || current_monitor != monitor);
    self.associations.len() != before
  }
}

#[test]
fn actor_classification_traits_match_actor_ref_contracts() {
  fn assert_actor_bus<T: ActorEventBus + ActorClassifier + ManagedActorClassification>(_bus: &T) {}

  let mut bus = ActorBus::default();
  let monitored = actor_ref(1);
  let monitor = actor_ref(2);

  assert_actor_bus(&bus);
  assert_eq!(<ActorBus as ActorEventBus>::compare_actor_subscribers(&monitored, &monitor), Ordering::Less);
  assert!(bus.subscribe(monitor.clone(), monitored.clone()));
  assert!(!bus.subscribe(monitor.clone(), monitored.clone()));
  assert_eq!(bus.associations.len(), 1);
  assert!(bus.unregister_from_unsubscriber(&monitor, 1));
  assert!(bus.unsubscribe(&monitor, &monitored));
  assert!(bus.associations.is_empty());
}

#[test]
fn event_bus_test_adapters_exercise_unsubscribe_and_noop_contracts() {
  let mut lookup = LookupBus::default();
  assert!(lookup.subscribe(1, "a"));
  assert!(lookup.subscribe(2, "b"));
  lookup.unsubscribe_all(&1);
  assert_eq!(lookup.subscriptions, [("b", 2)]);

  let mut subchannel = SubchannelBus::default();
  assert!(subchannel.subscribe(1, "a"));
  assert!(!subchannel.subscribe(1, "a"));
  assert!(subchannel.unsubscribe(&1, &"a"));
  assert!(subchannel.subscribe(2, "b"));
  subchannel.unsubscribe_all(&2);
  assert!(subchannel.subscriptions.is_empty());

  let mut scanning = ScanningBus::default();
  assert!(scanning.subscribe(1, 3));
  assert!(!scanning.subscribe(1, 3));
  assert!(scanning.unsubscribe(&1, &3));
  assert!(scanning.subscribe(2, 4));
  scanning.unsubscribe_all(&2);
  assert!(scanning.subscriptions.is_empty());

  fn positive(value: &u32) -> bool {
    *value > 0
  }
  let classifier: fn(&u32) -> bool = positive;
  assert!(classifier(&1));
  let mut predicate = PredicateBus;
  assert!(predicate.subscribe(1, classifier));
  assert!(predicate.unsubscribe(&1, &classifier));
  predicate.unsubscribe_all(&1);
  predicate.publish(1);

  let mut actor_bus = ActorBus::default();
  let monitored = actor_ref(10);
  let monitor = actor_ref(11);
  assert_eq!(actor_bus.map_size(), 128);
  assert_eq!(actor_bus.classify(&()).pid(), actor_ref(0).pid());
  assert!(actor_bus.register_with_unsubscriber(&monitor, 1));
  assert!(actor_bus.subscribe(monitor.clone(), monitored.clone()));
  actor_bus.publish(());
  actor_bus.unsubscribe_all(&monitor);
  assert!(actor_bus.associations.is_empty());
}

fn actor_ref(value: u64) -> ActorRef {
  ActorRef::new_with_builtin_lock(Pid::new(value, 0), NullSender)
}
