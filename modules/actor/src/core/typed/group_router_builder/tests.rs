use crate::core::typed::{group_router_builder::GroupRouterBuilder, service_key::ServiceKey};

#[test]
fn should_create_builder_from_service_key() {
  let key = ServiceKey::<u32>::new("test-group");
  let _builder = GroupRouterBuilder::new(key);
}

// --- 統合テスト計画 ---
//
// `GroupRouterBuilder::build()` は `TypedActorRef<ReceptionistCommand>` を引数に取り、
// 完全なアクターシステム（ActorSystem + Receptionist アクター）が必要なため
// 単体テストでは検証できない。以下の振る舞いを統合テストで検証すべき:
//
// 1. build() が有効な Behavior<M> を返すこと
// 2. Receptionist への Subscribe コマンドが送信されること
// 3. Listing 更新時に routee セットが更新されること
// 4. ラウンドロビンでメッセージがルーティングされること
// 5. routee が空の場合にメッセージがドロップされること（パニックしないこと）
// 6. routee 追加・削除後もラウンドロビンが正しく動作すること
