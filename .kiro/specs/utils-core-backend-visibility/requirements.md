# 要件ドキュメント

## 導入
`fraktor-utils-core` のコレクション型（`SyncQueue`, `SyncStack`, `AsyncQueue`, `AsyncStack`）では、これまで Backend 実装型や Backend trait が公開 API のシグネチャに直接登場しており、AI やユーザーが内部詳細へ依存できてしまう状態だった。本仕様では Backend 型を完全に非公開化し、コレクションのパラメータとしては `BackendMeta` のみを公開する。`BackendMeta` は公開型だが Backend への参照を一切含まず、crate 内部のリゾルバが `BackendMeta` から適切な Backend を生成する。これにより型安全性と柔軟性を維持しながら、内部実装の漏洩を防ぐ。

## 要件

### 要件1: Backend型の完全隠蔽
**目的:** 公開 API から Backend 実装・trait を完全に排除し、外部コードが Backend に依存できないようにする。

#### 受け入れ条件
1. `SyncQueue`, `SyncStack`, `AsyncQueue`, `AsyncStack` の公開定義は `pub struct SyncQueue<T, K, BackendMetaParam = meta::VecDequeSyncQueueMeta>` のように `BackendMetaParam`（公開 `BackendMeta` パラメータ）のみを公開し、Backend 実体を束縛する `BM` は `crate::collections::sync_queue::inner` などの内部モジュールへ移し、`pub(crate)` もしくは `pub(super)` に制限しなければならない。
2. Backend trait（`QueueBackend`, `StackBackend`, `AsyncQueueBackend`, `AsyncStackBackend`）および実装型（`VecDequeBackend`, `HeaplessQueueBackend`, `TokioQueueBackend` 等）は、`pub(crate)` 以下の可視性で定義され、`pub use` やドキュメント化された API として露出してはならない。
3. 内部コード（同クレート内の別モジュールやテスト）は `crate::backend::...` の FQCN で Backend にアクセスできるが、`utils-core` クレート外から同じ型へアクセスする手段は存在してはならない。
4. 公開 API の rustdoc が Backend を示唆する場合は修正され、`BackendMeta` を利用した記述のみになるよう維持しなければならない。

### 要件2: BackendMeta型の公開と Backend からの分離
**目的:** 利用者に公開する `BackendMeta` を Backend と独立した純粋なメタデータとして定義し、内部でのみ Backend を解決できるようにする。

#### 受け入れ条件
1. `BackendMeta` は `pub trait BackendMeta: sealed::Sealed + Copy + 'static` のように公開されるが、trait のシグネチャには Backend trait・Backend 型・内部モジュールへの参照を含めてはならない（例: `type Backend` や `fn backend()` などは禁止）。
2. 各 `BackendMeta` 具体型（`VecDequeSyncQueueMeta`, `HeaplessSyncQueueMeta`, `TokioAsyncQueueMeta` 等）は ZST の `pub struct` とし、1 ファイル 1 型ルールに従って個別ファイルへ配置しなければならない。
3. `BackendMeta` は Queue/Stack の種別や必要なランタイム（`no_std`, `std`, `tokio` 等）、容量制限など “公開しても問題のない” メタデータのみを保持し、Backend のメソッドや型を返す API を持ってはならない。
4. `BackendMeta` の実装では、Backend 型を露出する関連型・戻り値・フィールド・定数を定義してはならず、Backend インスタンスを生成または返却するメソッドや `const`/`fn` を公開してはならない（必要な生成は `BackendMetaResolver` など crate 内部の補助で完結させる）。
   - **備考:** `BackendMeta` 自体の public API で Backend 型が参照されなければよい。crate 内の private trait 実装で Backend 型を記述するのは許容される。
5. `BackendMeta` と Backend の紐付けは `pub(crate)` の `BackendMetaResolver` モジュール（もしくは関数群）で集中管理し、`match meta.discriminant()` や `if TypeId::of::<BackendMetaParam>() == ...` のような内部分岐で Backend 実装を選択しなければならない。`BackendMeta` 側が `BackendMetaResolver` の trait を実装するような形式は禁止とし、Resolver が `BackendMeta` へ依存する一方向構造を保つ。
6. `SyncQueue`, `SyncStack`, `AsyncQueue`, `AsyncStack` のコンストラクタ（`new`, `with_capacity` 等）は `BackendMetaParam: BackendMeta` を受け取り、内部で `BackendMetaResolver::resolve(meta)` のような関数を呼んで Backend を生成しなければならない。

### 要件3: AI誤用の防止
**目的:** AI エージェントや利用者が Backend へ触れずに API を完結できるようにし、誤用を構造的に排除する。

#### 受け入れ条件
1. コレクション利用コードにおいて、型推論・タurbofish いずれのケースでも Backend 型を指定する必要がなく、`SyncQueue::<i32, VecDequeSyncQueueMeta>::new(VecDequeSyncQueueMeta)` のように公開 `BackendMeta` 型のみで型指定が完結しなければならない。
2. 補完や rustdoc に Backend 型名が表示されないよう、`BackendMeta` のみを公開し、Backend 名は `#[doc(hidden)]` 等で露出を防ぐ必要がある。
3. Doc コメント・ガイド・サンプルコードは `BackendMeta` と `BackendMetaResolver`（内部マッチングによる紐付け）の仕組みを強調し、Backend 名や内部モジュール名を記述してはならない。
4. `BackendMeta` のみが IDE のシグネチャ補完に現れるように `pub use meta::{...};` を整理し、Backend 系モジュールは `pub(crate)` のままにしておかなければならない。

### 要件4: ドキュメント・例の整備
**目的:** 利用者が `BackendMeta` を使ったパターンのみを見て迷わないようにする。

#### 受け入れ条件
1. `SyncQueue`, `SyncStack`, `AsyncQueue`, `AsyncStack` の rustdoc に `BackendMeta` を使った生成例を最低 1 つずつ掲載し、入力パラメータやランタイム条件も記載しなければならない。
2. Backend の説明は rustdoc から削除し、必要な情報は `BackendMeta` の rustdoc へ集約しなければならない。
3. `examples/` または crate ルートのドキュメントに、同期系・非同期系それぞれについて `BackendMeta` を使った end-to-end サンプルを用意し、`cargo run --example ...` で動作確認できなければならない。
4. 各 `BackendMeta` 具体型の rustdoc は英語で、「対応環境」「メモリ特性」など公開してよい情報のみを記述し、日本語コメントは禁止（プロジェクト規約遵守）。

### 要件5: テスト整合性の維持
**目的:** Backend 隠蔽後も内部実装と公開 API の両方を確実にテストする。

#### 受け入れ条件
1. 既存の `hoge/tests.rs` にある Backend 依存テストは内部モジュール経由で引き続き動作しなければならず、`pub(crate)` で隠蔽された Backend 型へアクセスできることを確認する。
2. `BackendMeta` を使った公開 API の新規テストケースを `sync_queue/tests.rs`, `async_queue/tests.rs` などに追加し、`new`, `with_capacity`, `push/pop` など代表的操作が `BackendMeta` のみで成立することを保証しなければならない。
3. 統合テスト（`crate/tests/*.rs`）や doctest も `BackendMeta` ベースに書き換え、Backend 名が 1 行も出現しないことを CI で検証する（`rg` などによる禁止ワードチェックを導入してもよい）。
4. `scripts/ci-check.sh all` が Backend 隠蔽後も成功することを確認し、必要に応じて `no_std`, `std`, `tokio` 各構成のテストを更新しなければならない。
5. Backend 実装を差し替えた際にも公開 API テストが壊れないよう、`BackendMetaResolver` の内部 `match/if` をフックできるテスト支援コードを用意しなければならない（crate 内限定）。

### 要件6: Lint整合性の保持
**目的:** `type-per-file-lint` や `module-wiring-lint` 等の独自ルールに従い、構造の一貫性を保つ。

#### 受け入れ条件
1. Backend 型および `BackendMeta` 具体型は必ず 1 ファイル 1 型で配置し、`backend/vec_deque.rs`, `backend_meta/vec_deque_sync_queue_meta.rs` のように命名しなければならない。
2. `BackendMeta` trait とその具体型は `pub use crate::collections::sync_queue::meta::{VecDequeSyncQueueMeta, ...};` のように公開モジュール階層から再エクスポートし、`module-wiring-lint` に従う。
3. Backend 実装ファイルの可視性は `pub(crate)` 以下であることを明示し、`use` 文の順序・グルーピングは `use-placement-lint` を満たす形で整理する。
4. Backend 関連テストは `backend/tests.rs` や `backend_meta/tests.rs` に配置し、`tests-location-lint` の要件（対象モジュールと同名ディレクトリ）に従わなければならない。
5. rustdoc コメントは英語、それ以外のコメントは日本語というプロジェクト規約を Backend/BackendMeta ファイルでも徹底し、リンタ警告が 0 件であることを CI で確認しなければならない。
