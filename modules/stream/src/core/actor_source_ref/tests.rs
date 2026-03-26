use crate::core::{ActorSourceRef, BoundedSourceQueue, OverflowStrategy, QueueOfferResult, StreamError};

// --- tell ---

#[test]
fn actor_source_ref_tell_enqueues_value() {
  // 準備: 容量4のキューに紐づく ActorSourceRef
  let queue = BoundedSourceQueue::new(4, OverflowStrategy::Fail);
  let mut source_ref = ActorSourceRef::new(queue);

  // 実行: tell で値を送信
  let result = source_ref.tell(42_u32);

  // 検証: 値がエンキューされる
  assert_eq!(result, QueueOfferResult::Enqueued);
}

#[test]
fn actor_source_ref_tell_respects_overflow_strategy() {
  // 準備: 容量1・Fail戦略のキューに紐づく ActorSourceRef
  let queue = BoundedSourceQueue::new(1, OverflowStrategy::Fail);
  let mut source_ref = ActorSourceRef::new(queue);

  // 実行: バッファ容量を超える値を送信
  let first = source_ref.tell(1_u32);
  let second = source_ref.tell(2_u32);

  // 検証: 1つ目は成功、2つ目は BufferOverflow で失敗
  assert_eq!(first, QueueOfferResult::Enqueued);
  assert_eq!(second, QueueOfferResult::Failure(StreamError::BufferOverflow));
}

#[test]
fn actor_source_ref_tell_returns_queue_closed_after_complete() {
  // 準備: 完了済みの ActorSourceRef
  let queue = BoundedSourceQueue::new(4, OverflowStrategy::Fail);
  let mut source_ref = ActorSourceRef::new(queue);
  source_ref.complete();

  // 実行: 完了後に tell を試行
  let result = source_ref.tell(1_u32);

  // 検証: QueueClosed が返される
  assert_eq!(result, QueueOfferResult::QueueClosed);
}

// --- complete ---

#[test]
fn actor_source_ref_complete_closes_queue() {
  // 準備: オープン状態の ActorSourceRef
  let queue = BoundedSourceQueue::<u32>::new(4, OverflowStrategy::Fail);
  let mut source_ref = ActorSourceRef::new(queue);
  assert!(!source_ref.is_closed());

  // 実行: 完了
  source_ref.complete();

  // 検証: キューが閉じる
  assert!(source_ref.is_closed());
}

// --- fail ---

#[test]
fn actor_source_ref_fail_closes_queue_with_error() {
  // 準備: オープン状態の ActorSourceRef
  let queue = BoundedSourceQueue::new(4, OverflowStrategy::Fail);
  let mut source_ref = ActorSourceRef::new(queue);

  // 実行: エラーで失敗させる
  source_ref.fail(StreamError::Failed);

  // 検証: キューが閉じ、以降の tell は失敗を報告
  assert!(source_ref.is_closed());
  assert_eq!(source_ref.tell(1_u32), QueueOfferResult::Failure(StreamError::Failed));
}

// --- is_closed ---

#[test]
fn actor_source_ref_is_closed_returns_false_when_open() {
  // 準備: 新しく作成した ActorSourceRef
  let queue = BoundedSourceQueue::<u32>::new(4, OverflowStrategy::Fail);
  let source_ref = ActorSourceRef::new(queue);

  // 検証: is_closed は false
  assert!(!source_ref.is_closed());
}

// --- Clone ---

#[test]
fn actor_source_ref_clone_shares_queue() {
  // 準備: ActorSourceRef とそのクローン
  let queue = BoundedSourceQueue::new(4, OverflowStrategy::Fail);
  let mut source_ref = ActorSourceRef::new(queue);
  let mut cloned = source_ref.clone();

  // 実行: オリジナル経由で tell、結果を検証
  assert_eq!(source_ref.tell(1_u32), QueueOfferResult::Enqueued);

  // 実行: クローン経由で完了
  cloned.complete();

  // 検証: 両方が閉じた状態を確認
  assert!(source_ref.is_closed());
  assert!(cloned.is_closed());
}
