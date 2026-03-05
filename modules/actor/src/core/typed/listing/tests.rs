use alloc::vec;
use core::any::TypeId;

use crate::core::typed::listing::Listing;

#[test]
fn listing_should_store_fields() {
  let listing = Listing::new("svc", TypeId::of::<u32>(), vec![]);
  assert_eq!(listing.service_id(), "svc");
  assert_eq!(listing.type_id(), TypeId::of::<u32>());
  assert!(listing.is_empty());
}

// --- 統合テスト計画 ---
//
// `Listing::typed_refs::<M>()` は `ActorRef` → `TypedActorRef<M>` への変換を行う。
// `ActorRef` の生成にはアクターシステムが必要なため、単体テストでは検証できない。
// 以下の振る舞いを統合テストで検証すべき:
//
// 1. typed_refs() が登録済み ActorRef を TypedActorRef<M> に変換すること
// 2. 複数の ActorRef が正しく変換されること
// 3. refs() と typed_refs() の要素数が一致すること
