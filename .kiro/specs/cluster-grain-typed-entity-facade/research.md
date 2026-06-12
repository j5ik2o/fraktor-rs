# 調査・設計判断

## 要約

- **機能**: `cluster-grain-typed-entity-facade`
- **ディスカバリー範囲**: 拡張（既存 untyped grain API への typed facade 追加）
- **主要な発見**:
  - typed 識別は `cluster-core-typed` の `ClusterIdentity<M>`（`PhantomData<fn() -> M>`）として既に存在し、新設不要。欠けているのは typed な参照（`GrainRef<M>` 相当）と取得経路のみ
  - kernel `GrainRef` の呼び出し面は `tell_with_sender` / `request` / `request_future` / `with_options` / `with_codec` で完結しており、typed 層は薄い委譲で全要件を満たせる
  - 応答の型付けに必要な `TypedAskResponse::from_generic` / `TypedAskFuture::new` が `pub(crate)` のため、actor-core-typed に公開コンストラクタ（`from_untyped`）の追加が必要

## 調査ログ

### kernel grain API の公開面

- **背景**: typed wrapper が委譲すべき untyped API の正確な面を確定する
- **参照した情報源**: `modules/cluster-core-kernel/src/grain/grain_ref.rs`, `grain_key.rs`, `grain_call_options.rs`, `grain_codec.rs`, `modules/cluster-core-kernel/src/activation/cluster_identity.rs`, `modules/cluster-core-kernel/src/extension/cluster_api.rs`
- **発見**:
  - `GrainRef::new(api: ClusterApi, identity: ClusterIdentity)`。`ClusterApi` は `#[derive(Clone)]`
  - 呼び出し: `request(&AnyMessage) -> Result<AskResponse, GrainCallError>`、`request_future -> Result<ActorFutureShared<AskResult>, GrainCallError>`、`tell_with_sender(&AnyMessage, &ActorRef)`、`request_with_sender`
  - 構成: `with_options(GrainCallOptions)`（timeout / retry）、`with_codec(ArcShared<dyn GrainCodec>)`（任意。設定時はラウンドトリップ検証）
  - `ClusterIdentity::new(kind, identity) -> Result<_, ClusterIdentityError>` が空文字等を拒否。`key()` が `"{kind}/{identity}"` の `GrainKey` を導出
  - `ClusterApi::try_from_system -> Result<_, ClusterApiError>`。`ClusterApiError` は `ExtensionNotInstalled` のみ
- **含意**: kernel は無変更でよい。typed 層は識別・参照・取得経路の3点を包むだけで要件 1〜4 を満たせる

### typed wrapper の先行パターン

- **背景**: typed 層の構造・命名・変換 API を既存パターンに揃える（learning-before-coding）
- **参照した情報源**: `modules/actor-core-typed/src/actor_ref.rs`, `modules/actor-core-typed/src/system.rs`, `modules/cluster-core-typed/src/cluster_identity.rs`, `modules/cluster-core-typed/src/cluster.rs`
- **発見**:
  - `TypedActorRef<M>` = `{ inner: ActorRef, _marker: PhantomData<M> }` + `from_untyped` / `as_untyped` / `into_untyped`
  - typed `ClusterIdentity<M>` = `{ inner: KernelClusterIdentity, _message: PhantomData<fn() -> M> }` + `from_kernel` / `as_kernel` / `into_kernel`（kernel 系は `*_kernel` 命名）
  - `Cluster` facade = `{ inner: ClusterApi }`、`Cluster::get<M>(system: &TypedActorSystem<M>)` が `ClusterApi::try_from_system(system.as_untyped())` へ委譲
  - lib.rs は `mod` 宣言 + 最小 `pub use`（module-wiring-lint 準拠）、テストは sibling `*_test.rs`
- **含意**: typed `GrainRef<M>` は kernel `GrainRef` を包む同名型とし、変換 API は `from_kernel` / `as_kernel` / `into_kernel`（cluster-core-typed の既存命名）に揃える

### 応答の型付け経路

- **背景**: `request` の戻り値 `AskResponse` / `ActorFutureShared<AskResult>` を型付きで返す手段の確認
- **参照した情報源**: `modules/actor-core-typed/src/dsl/typed_ask_response.rs`, `typed_ask_future.rs`, `modules/actor-core-kernel/src/actor/messaging/ask_response.rs`, `any_message.rs`
- **発見**:
  - `TypedAskResponse<R>` / `TypedAskFuture<R>` が untyped 応答の型付け（downcast + `try_unwrap`）を既に実装。エラーは `TypedAskError`（`TypeMismatch` / `SharedReferences` / `AskFailed`）
  - ただしコンストラクタ `from_generic` / `new` は `pub(crate)` で、cluster-core-typed から利用できない
  - `AnyMessage::new<T: Any + Send + Sync + 'static>` がメッセージ構築の型束縛を決める
- **含意**: cluster-core-typed で応答型付けを再実装すると同一意図の重複になる（intent-based-dedup 違反）。actor-core-typed に公開コンストラクタ `from_untyped` を追加して再利用する

### Pekko EntityTypeKey / EntityRef の最小面

- **背景**: 参照実装の公開 API 最小面を確認し、過剰設計を避ける
- **参照した情報源**: `references/pekko/cluster-sharding-typed/src/main/scala/.../scaladsl/ClusterSharding.scala`
- **発見**:
  - `EntityTypeKey[-T]` は `name: String` のみの薄い宣言点。`EntityRef[-M]` は `entityId` / `typeKey` / `dataCenter` / `tell` / `ask` / `askWithStatus`
  - 生成元は `ClusterSharding#entityRefFor(typeKey, entityId)`（extension からの取得経路）
- **含意**: fraktor では `ClusterIdentity<M>` が kind + entity id の合成識別を既に持つ。Pekko の TypeKey 相当は「kind を宣言しメッセージ型と紐づける」最小の宣言点として `GrainTypeKey<M>` を追加し、`identity_for(entity_id)` で `ClusterIdentity<M>` を導出する形が最小面

### CONTEXT.md 語彙衝突の確認

- **背景**: brief の制約「CONTEXT.md の語彙と衝突しないよう先に用語を確定する」
- **参照した情報源**: `CONTEXT.md`
- **発見**: Grain / Entity / typed 系の語彙は未定義（cluster membership / failure detector 系のみ）。衝突なし
- **含意**: 型名対応（Pekko `EntityTypeKey[M]` ↔ `GrainTypeKey<M>`、`EntityRef[M]` ↔ typed `GrainRef<M>`）は design.md に明記する。CONTEXT.md は実装詳細の型名一覧を載せない方針のため追加しない

## アーキテクチャパターン評価

| 選択肢 | 説明 | 強み | リスク／制約 | メモ |
|--------|-------------|-----------|---------------------|-------|
| 薄い typed facade（採用） | kernel `GrainRef` を `PhantomData<fn() -> M>` で包み委譲 | 既存パターンと一致、kernel 無変更、zero-cost | 応答型付けに actor-core-typed の公開面拡大が必要 | `ClusterIdentity<M>` / `TypedActorRef<M>` と同型 |
| typed 専用ロジックを cluster-core-typed に実装 | 応答 downcast 等を独自実装 | actor-core-typed 無変更 | `TypedAskFuture` と同一意図の重複実装 | intent-based-dedup 違反のため却下 |
| kernel に型パラメータを導入 | kernel `GrainRef<M>` 化 | 層が1つ減る | kernel 全利用箇所の破壊的変更、untyped API 消失 | brief の「kernel 無変更」制約違反のため却下 |

## 設計判断

### 判断: typed 識別は既存 `ClusterIdentity<M>` を再利用し、宣言点として `GrainTypeKey<M>` のみ追加する

- **背景**: 要件 1（型付き Grain 識別契約）と Pekko `EntityTypeKey` 相当の宣言点
- **検討した代替案**:
  1. `ClusterIdentity<M>` だけで済ませる — 宣言点がなく、kind 文字列が呼び出し箇所に分散する
  2. Pekko 同様に TypeKey と EntityRef を完全分離した新型体系 — 既存 `ClusterIdentity<M>` と重複する
- **採用したアプローチ**: `GrainTypeKey<M>`（kind + `PhantomData<fn() -> M>`）を新設し、`identity_for(entity_id) -> Result<ClusterIdentity<M>, ClusterIdentityError>` で既存 typed 識別へ合流させる
- **根拠**: 「kind とメッセージ型の対応を一箇所で宣言する」という Pekko TypeKey の本質だけを最小型で導入し、識別の正本は既存型に保つ
- **トレードオフ**: 型が1つ増えるが、kind 文字列の分散と型不整合の温床を防ぐ
- **フォローアップ**: cluster-sharding-extractor-contract が `GrainTypeKey<M>` を参照する前提を design に明記

### 判断: 応答型付けは actor-core-typed の `TypedAskResponse<R>` / `TypedAskFuture<R>` を公開コンストラクタ追加で再利用する

- **背景**: 要件 2.4 / 2.5（request / request_future の型付き応答）
- **検討した代替案**:
  1. cluster-core-typed に独自の typed 応答型を新設 — downcast / try_unwrap ロジックの重複
  2. untyped `AskResponse` をそのまま返す — 型安全の要件を満たさない
- **採用したアプローチ**: `TypedAskResponse::from_untyped(AskResponse)` / `TypedAskFuture::from_untyped(ActorFutureShared<AskResult>)` を public で追加し、cluster-core-typed から利用する
- **根拠**: 同一意図（untyped 応答の型付け）の正本を1つに保つ。命名は `TypedActorRef::from_untyped` の前例に一致
- **トレードオフ**: actor-core-typed の公開面が2メソッド増える（typed facade 系 crate の正規経路として正当化）
- **フォローアップ**: 公開コンストラクタの rustdoc に「typed facade crate 向けの変換点」であることを明記

### 判断: 相互変換は明示メソッドのみとし、`From` / `Into` 実装を提供しない

- **背景**: 要件 4.3（暗黙変換の禁止）
- **採用したアプローチ**: `from_kernel` / `as_kernel` / `into_kernel` の3点のみ（cluster-core-typed の既存命名に一致）
- **根拠**: untyped→typed はメッセージ型の表明を伴うため、呼び出し箇所で型を明示させる
- **トレードオフ**: 変換の記述が冗長になるが、誤った型表明の混入点を可視化できる

### 判断: 型安全性の検証は rustdoc `compile_fail` テストで行う

- **背景**: 要件 2.2「それ以外の型の送信をコンパイル時に拒否」の検証手段
- **採用したアプローチ**: typed `GrainRef<M>` の rustdoc に `compile_fail` doctest を置く（`cluster_provider_shared.rs` の既存前例に従う）
- **根拠**: コンパイル時拒否は実行時テストで検証できないため、コンパイル失敗自体をテストにする

## リスクと緩和策

- `TypedAskError::SharedReferences`（future の多重 clone 時に応答取り出し失敗）— typed `GrainRef` の rustdoc に単一消費者前提を明記し、テストで単一取り出し経路を検証
- actor-core-typed の公開面拡大が他の typed facade に波及 — rustdoc で用途（typed facade crate の変換点）を限定し、`#[must_use]` を付与
- 統合テストで grain runtime（activation）の起動手順が複雑 — kernel `grain_ref` の既存テスト構成（`ClusterApi::try_from_system` + kind 登録）を踏襲して最小化

## 参考資料

- `references/pekko/cluster-sharding-typed/.../scaladsl/ClusterSharding.scala` — EntityTypeKey / EntityRef / entityRefFor の最小面
- `docs/gap-analysis/cluster-gap-analysis.md` — カテゴリ8 easy 項目（本 spec の起点）
- `.kiro/specs/cluster-grain-typed-entity-facade/brief.md` — discovery 決定事項
