# Gap Analysis: utils-core-backend-visibility

- `SyncQueue`/`AsyncQueue`/`SyncStack` に加え、`Sync/Async` 各種 Producer/Consumer（例: `SyncMpscProducer`, `SyncSpscConsumer`, `AsyncMpscProducer`, `AsyncSpscConsumer`）も `B: BackendTrait` をそのまま公開し、`SharedVecRingQueue` などの型 alias も Backend 型を露出している。よって Req1（Backend 完全隠蔽）は未着手。
- `BackendMeta` や `BackendMetaResolver` 相当のモジュールは存在せず、コンストラクタも `ArcShared<Mutex<Backend>>` を直接受け取っているため、Req2〜3 のメタ情報運用や AI 誤用防止ができていない。
- Mailbox（`modules/actor-core/src/mailbox/*.rs`）を含む利用箇所が `VecRingBackend` を直生成しているので、`utils-core` だけでなく `actor-core` にも波及する破壊的変更が必要。
- テスト・ドキュメントは backend 名を前提にした記述ばかりで、`BackendMeta` 経由の例／検証がゼロ。ドキュメント刷新と禁止語チェックの導入が必要。

- **公開構造**: `modules/utils-core/src/collections/queue.rs` と `stack.rs` が backend trait/実装を `pub` 再輸出。`SharedVecRingQueue` / `SharedVecStack` が backend 型 alias を公開している。
- **API 形態**: `SyncQueue<T,K,B,M>`（`sync_queue.rs:23-209`）、`AsyncQueue<T,K,B,A>`、`SyncStack<T,B,M>` に加え、`SyncMpscProducer/Consumer`, `SyncSpscProducer/Consumer`, `AsyncMpscProducer/Consumer`, `AsyncSpscProducer/Consumer` が backend ジェネリックを直接提示し、`shared()` で `ArcShared<Mutex<B>>` や `ArcShared<AsyncMutex<B>>` を返す。
- **利用側**: `actor-core` の Mailbox が `VecRingBackend` と `VecRingStorage` を直接構築し、`QueueMutex<T, TB> = ToolboxMutex<VecRingBackend<T>, TB>` と型 alias 済み（`mailbox.rs:5-49`, `mailbox_queue_handles.rs:5-59`）。
- **テスト/サンプル**: `queue/tests.rs` や `stack/tests.rs` ではバックエンド実装をそのままnewし、`SyncQueue<_, FifoKey, VecRingBackend<_>, _>` のように型注釈している。`examples/` も backend を前提。
- **共通インフラ**: `ArcShared` + `SyncMutexLike` による共有、`WaitQueue` による Block ポリシー待機、`RuntimeToolbox::MutexFamily` などの抽象は維持されているため、`BackendMeta` からこれらを組み立てるフックが必要。

## 3. 要件⇔資産マップ

| 要件 | 既存資産 | 状態 | ギャップ | 対応の着想 |
| --- | --- | --- | --- | --- |
| Req1 Backend 隠蔽 | `queue.rs`/`stack.rs` `backend/*.rs` `shared` alias | Missing | Backend/trait/alias が `pub` のまま。`Sync/Async Queue/Stack` と各 Producer/Consumer が backend を公開。 | `collections/queue` と `stack` に `inner` モジュールを追加し、公開 API から backend ジェネリックを排除。`backend` モジュールは `pub(crate)` へ。 |
| Req2 BackendMeta 導入 | 該当なし | Missing | `BackendMeta` `BackendMetaResolver` ファイルが存在しない。 | `collections/queue/meta/` `stack/meta/` ディレクトリを新設し、ZST `BackendMeta` 実装＋ Resolver 関数を配置。 |
| Req3 AI 誤用防止 | ドキュメント/型 alias/テスト | Missing | rustdoc/補完で backend 名が表に出る。 | `BackendMeta` を `pub use meta::*;` で再輸出、backend 名は `#[doc(hidden)]` または非公開。 |
| Req4 ドキュメント・例 | `queue/tests.rs`, README, specs | Missing | `BackendMeta` を使った記述が皆無。 | 新サンプル `examples/backend_meta_queue.rs` などを追加し、rustdoc サンプルを `BackendMeta` ベースに差し替え。 |
| Req5 テスト整合 | queue/stack tests, actor-core integration | Missing | BackendMeta 経路のテストがない。 | `queue/tests.rs` や `actor-core` 統合テストに `BackendMeta` 指定で `new` するケースを追加。Resolver をモック化するテスト支援を実装。 |
| Req6 Lint 整合 | `type-per-file-lint` など | Constraint | 新 `meta` モジュールで 1 型 1 ファイルを守る必要。 | `backend_meta/vec_deque_sync_queue_meta.rs` 等の命名を徹底し、`module-wiring-lint` に沿う再輸出を設計。 |

## 4. 実装アプローチ候補

### Option A: 既存 API をラップして backend を `inner` へ移動
- `SyncQueue` など公開型は `struct SyncQueue<T,K, Meta>` として `ArcShared<InnerQueue>` を保持。`InnerQueue`（`pub(crate)`）が従来の `SyncQueue<T,K,B,M>` を wrap。
- **利点**: 現行 backend 実装を最大限再利用でき、Mailbox など利用側の置き換えが最小。
- **欠点**: Wrapper 層が増えて `ArcShared` と `SharedAccess` の適用箇所が二重化。`inner` API の可視性制御が煩雑。

### Option B: 新 `QueueBuilder`/`StackBuilder` + `BackendMetaResolver` を定義
- `BackendMeta` ごとに `QueueConfig` を生成し、Resolver 側で `match meta.discriminant()` により backend を決定して `ArcShared` を構築。`SyncQueue` などは Builder が組み立てた `SharedBackendHandle` のみを保持。
- **利点**: 公開 API が “Meta → Queue” に統一され、doc/サンプルでの説明が明確。`BackendMeta` 追加も Builder 拡張で完結。
- **欠点**: 既存 `SyncQueue` 型の署名が大きく変わるため、`utils-core` 利用者は全面改修。`SyncMpscProducer` 等も Builder 発のハンドルに差し替えが必要。

### Option C: 段階的ハイブリッド
- フェーズ1で `BackendMeta`/Resolver/Builder を追加し、旧 API は `pub(crate)` 化 + `#[deprecated]` (内部限定) に変更。`actor-core` などを順次新 API へ移行後、旧構造を削除。
- **利点**: 破壊的変更を段階導入でき、テストもフェーズ毎に更新可能。
- **欠点**: 移行期間中は旧 API と新 API が並存し、lint/ドキュメントの整合管理が難しい。

## 5. Effort / Risk
- **Effort: L (1〜2 週間)** — queue/stack/async 全系統 API + Mailbox など利用側の置換に加え、docs/tests/lint 調整が必要。
- **Risk: High** — `BackendMetaResolver` 設計を誤ると no_std や `RuntimeToolbox` 依存が破綻する恐れ。既存公開 API を全面破壊するためレビュー負荷も大きい。

## 6. Research Needed
1. **Resolver 実装方式**: no_std で `TypeId` を利用できるか、もしくは `BackendMeta` に enum 的 discriminant を保持させるべきか。
2. **RuntimeToolbox 連携**: `Mailbox` のように `ToolboxMutex` を使う利用側で `BackendMeta` から適切な MutexFamily を選ぶ方法。
3. **Meta バリエーション**: VecRing/VecStack/Tokio/heapless など想定する BackendMeta の一覧と公開可能なメタデータ項目。
4. **Docs/Lint 連動**: `module-wiring-lint`/`type-per-file-lint` に適合する `meta`/`resolver` ディレクトリ構造の詳細ルール。

## 7. 次のステップ
- `/prompts:kiro-spec-design utils-core-backend-visibility` を実行し、上記オプションと Research 項目を踏まえた設計ドキュメントを作成する。
- Resolver 設計・Builder API・Mailbox 影響範囲を詳細化し、テスト更新計画（BackendMeta 経路の単体/統合テスト）を整理する。
