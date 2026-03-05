//! Builder for configuring and constructing group routers.

#[cfg(test)]
mod tests;

use alloc::{vec, vec::Vec};
use core::sync::atomic::AtomicUsize;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};
use portable_atomic::Ordering;

use crate::core::typed::{
  actor::TypedActorRef, behavior::Behavior, behaviors::Behaviors, listing::Listing, receptionist::Receptionist,
  receptionist_command::ReceptionistCommand, service_key::ServiceKey,
};

/// Configures and builds a group router behavior.
///
/// Unlike a pool router that spawns its own children, a group router discovers
/// routees dynamically via the Receptionist service.  It subscribes to listing
/// changes for the provided [`ServiceKey`] and updates its routee set
/// accordingly.
pub struct GroupRouterBuilder<M>
where
  M: Send + Sync + Clone + 'static, {
  service_key: ServiceKey<M>,
}

impl<M> GroupRouterBuilder<M>
where
  M: Send + Sync + Clone + 'static,
{
  /// Creates a new group router builder for the given service key.
  pub(crate) const fn new(service_key: ServiceKey<M>) -> Self {
    Self { service_key }
  }

  /// Builds the group router as a [`Behavior`].
  ///
  /// The router subscribes to listing changes for the configured service key
  /// via the Receptionist and routes messages to discovered actors using
  /// round-robin.
  ///
  /// **Important:** The caller must ensure a Receptionist actor is running
  /// and pass its reference via `receptionist_ref`.
  #[must_use]
  pub fn build(self, receptionist_ref: TypedActorRef<ReceptionistCommand>) -> Behavior<M> {
    let key = self.service_key;
    let routees: ArcShared<RuntimeMutex<Vec<TypedActorRef<M>>>> = ArcShared::new(RuntimeMutex::new(Vec::new()));
    let routees_for_listing = routees.clone();
    let routees_for_msg = routees;
    let receptionist = ArcShared::new(RuntimeMutex::new(receptionist_ref));

    Behaviors::setup(move |ctx| {
      // Create a child actor to receive Listing updates and refresh the routee set.
      let routees_updater = routees_for_listing.clone();
      let listing_factory = ArcShared::new(move || -> Behavior<Listing> {
        let ru = routees_updater.clone();
        Behaviors::receive_message(move |_ctx, listing: &Listing| {
          let typed_refs: Vec<TypedActorRef<M>> = listing.typed_refs();
          let mut guard = ru.lock();
          *guard = typed_refs;
          Ok(Behaviors::same())
        })
      });

      let listing_props =
        crate::core::typed::props::TypedProps::<Listing>::from_behavior_factory(move || (*listing_factory)());
      let listing_ref = match ctx.spawn_child(&listing_props) {
        | Ok(child) => child.actor_ref(),
        | Err(_) => return Behaviors::stopped(),
      };

      let subscribe_cmd = Receptionist::subscribe(&key, listing_ref);
      let _ = receptionist.lock().tell(subscribe_cmd);

      let rfm = routees_for_msg.clone();
      let index = AtomicUsize::new(0);
      Behaviors::receive_message(move |_ctx, message: &M| {
        let targets = {
          let guard = rfm.lock();
          if guard.is_empty() {
            return Ok(Behaviors::same());
          }
          let idx = index.fetch_add(1, Ordering::Relaxed) % guard.len();
          vec![guard[idx].clone()]
        };
        for mut target in targets {
          let _ = target.tell(message.clone());
        }
        Ok(Behaviors::same())
      })
    })
  }
}
