pub trait TypedActor<M>: Send + Sync
where
  M: Send + Sync + 'static, {
}
