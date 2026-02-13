# 要件ドキュメント

## 導入

本仕様は `modules/actor/src/core` および `modules/actor/src/std` 配下のサブパッケージを、protoactor-go と Apache Pekko の設計を参考にしながら改善するものである。

### 対象範囲
- `modules/actor/src/core/` 配下のサブパッケージ（actor, dispatch, supervision, typed, scheduler, messaging, event, props, dead_letter, serialization, spawn, extension, lifecycle, futures, system, error）
- `modules/actor/src/std/` 配下の対応するサブパッケージ

### 対象外
- `lib.rs`, `core.rs`, `std.rs` は変更しない
- サブパッケージの新規追加・削除は本仕様の対象外（既存サブパッケージの改善のみ）

### 制約
- 破壊的変更は許容される（後方互換性不要）
- 再エクスポート（`pub use super::*` 等）は禁止
- インクリメンタルな改善を行い、各変更ごとに `make ci-check` を通す
- 1ファイル1構造体/trait/enumの原則を遵守

### 参照実装
- protoactor-go: `references/protoactor-go/actor/`
- Apache Pekko: `references/pekko/actor/` および `references/pekko/actor-typed/`

---

## 要件

### 要件1: actor サブパッケージの構造改善
**目的:** ランタイム開発者として、アクターの基本プリミティブ（Pid, ActorRef, ActorPath, ActorContext, ActorCell）を明確に分離された責務で管理し、protoactor-go/pekkoのパターンに沿った一貫性を得たい。

#### 受け入れ条件
1. actor サブパッケージを変更したとき、各ファイルは単一の公開型（構造体/trait/enum）のみを含まなければならない
2. プライベートな補助型がある場合、同一ファイル内に配置するか、専用のサブモジュールに分離しなければならない
3. actor_path サブモジュールを変更したとき、ActorPath, ActorPathParts, ActorPathFormatter は独立したファイルに配置しなければならない
4. actor_ref サブモジュールを変更したとき、ActorRef と ActorRefGeneric は独立したファイルに配置しなければならない
5. actor_selection サブモジュールを変更したとき、ActorSelection と Resolver は独立したファイルに配置しなければならない
6. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件2: dispatch サブパッケージの構造改善
**目的:** ランタイム開発者として、Dispatcher と Mailbox の責務を protoactor-go の設計に沿って明確に分離し、メッセージ配信パスの理解と拡張を容易にしたい。

#### 受け入れ条件
1. dispatch サブパッケージを変更したとき、Dispatcher trait/実装と Mailbox trait/実装は独立したサブモジュールに配置しなければならない
2. dispatcher サブモジュールを変更したとき、Dispatcher trait と各実装（goroutine相当、synchronized相当）は独立したファイルに配置しなければならない
3. mailbox サブモジュールを変更したとき、Mailbox trait と各実装（bounded, unbounded）は独立したファイルに配置しなければならない
4. Dispatcher インターフェースは protoactor-go の `Schedule` と `Throughput` に相当するメソッドを持たなければならない
5. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件3: supervision サブパッケージの構造改善
**目的:** ランタイム開発者として、SupervisorStrategy と関連コンポーネント（Directive, RestartStatistics）を protoactor-go/pekko のパターンに沿って整理し、障害回復ロジックの拡張を容易にしたい。

#### 受け入れ条件
1. supervision サブパッケージを変更したとき、SupervisorStrategy trait は独立したファイルに配置しなければならない
2. SupervisorDirective enum は独立したファイルに配置しなければならない
3. RestartStatistics は独立したファイルに配置しなければならない
4. 具体的な戦略実装（OneForOneStrategy, AllForOneStrategy, RestartingStrategy）がある場合、各々独立したファイルに配置しなければならない
5. strategy.rs が複数の型を含む場合、1ファイル1型の原則に従って分割しなければならない
6. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件4: typed サブパッケージの構造改善
**目的:** ランタイム開発者として、Typed API（Behavior, TypedActorContext, TypedActorRef）を pekko actor-typed の設計に沿って整理し、Typed/Untyped 間の変換とシグナル処理を明確にしたい。

#### 受け入れ条件
1. typed サブパッケージを変更したとき、Behavior 関連の型（Behavior, BehaviorSignal, Behaviors）は各々独立したファイルに配置しなければならない
2. actor サブモジュールを変更したとき、TypedActor, TypedActorContext, TypedActorRef, TypedChildRef は各々独立したファイルに配置しなければならない
3. message_adapter サブモジュールを変更したとき、MessageAdapterRegistry と関連型は各々独立したファイルに配置しなければならない
4. scheduler サブモジュール内の型が複数ある場合、1ファイル1型の原則に従って分割しなければならない
5. Supervise ビルダーは独立したファイルに配置しなければならない
6. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件5: scheduler サブパッケージの構造改善
**目的:** ランタイム開発者として、Scheduler と TickDriver の責務を明確に分離し、pekko の Scheduler API パターンに沿った一貫性を得たい。

#### 受け入れ条件
1. scheduler サブパッケージを変更したとき、Scheduler コア（SchedulerCore, SchedulerRunner）と TickDriver サブシステムは独立したサブモジュールに分離しなければならない
2. tick_driver サブモジュールを変更したとき、TickDriver trait, TickDriverConfig, TickDriverBootstrap は各々独立したファイルに配置しなければならない
3. policy 関連の型（FixedDelayPolicy, FixedRatePolicy, PolicyRegistry）は独立したサブモジュールに配置しなければならない
4. diagnostics 関連の型（SchedulerDiagnostics, DiagnosticsRegistry）は独立したサブモジュールに配置しなければならない
5. cancellable 関連の型（CancellableEntry, CancellableRegistry, CancellableState）は独立したサブモジュールに配置しなければならない
6. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件6: messaging サブパッケージの構造改善
**目的:** ランタイム開発者として、メッセージ型（AnyMessage, SystemMessage, MessageEnvelope相当）と Ask パターン関連型を protoactor-go の設計に沿って整理したい。

#### 受け入れ条件
1. messaging サブパッケージを変更したとき、AnyMessage と AnyMessageGeneric は独立したファイルに配置しなければならない
2. SystemMessage は独立したファイルに配置しなければならない
3. AskError と AskResponse は各々独立したファイルに配置しなければならない
4. message_invoker サブモジュール内の型が複数ある場合、1ファイル1型の原則に従って分割しなければならない
5. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件7: event サブパッケージの構造改善
**目的:** ランタイム開発者として、EventStream と Logging サブシステムを明確に分離し、イベント購読と通知の拡張を容易にしたい。

#### 受け入れ条件
1. event サブパッケージを変更したとき、logging サブモジュールと stream サブモジュールは独立したディレクトリ構造を持たなければならない
2. logging サブモジュール内の型（LogEvent, LogLevel 等）は各々独立したファイルに配置しなければならない
3. stream サブモジュール内の型（EventStream, EventStreamEvent 等）は各々独立したファイルに配置しなければならない
4. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件8: props サブパッケージの構造改善
**目的:** ランタイム開発者として、Props と関連設定（MailboxConfig, SupervisorOptions, ActorFactory）を protoactor-go/pekko の設計に沿って整理し、アクター生成設定の拡張を容易にしたい。

#### 受け入れ条件
1. props サブパッケージを変更したとき、Props と PropsGeneric は独立したファイル（base.rs）に配置しなければならない
2. ActorFactory と ActorFactoryShared は各々独立したファイルに配置しなければならない
3. MailboxConfig と MailboxRequirement は各々独立したファイルに配置しなければならない
4. SupervisorOptions は独立したファイルに配置しなければならない
5. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件9: dead_letter サブパッケージの構造改善
**目的:** ランタイム開発者として、DeadLetter 処理と関連型（DeadLetterEntry, DeadLetterReason）を protoactor-go の設計に沿って整理し、配送不能メッセージの追跡を容易にしたい。

#### 受け入れ条件
1. dead_letter サブパッケージを変更したとき、DeadLetter と DeadLetterGeneric は独立したファイルに配置しなければならない
2. DeadLetterEntry と DeadLetterEntryGeneric は独立したファイルに配置しなければならない
3. DeadLetterReason は独立したファイルに配置しなければならない
4. DeadLetterShared は独立したファイルに配置しなければならない
5. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件10: serialization サブパッケージの構造改善
**目的:** ランタイム開発者として、シリアライゼーション機能（Serializer, Registry, Extension）を論理的なサブグループに整理し、シリアライザ拡張を容易にしたい。

#### 受け入れ条件
1. serialization サブパッケージを変更したとき、Serializer trait と SerializerWithStringManifest は独立したファイルに配置しなければならない
2. SerializationRegistry と関連型は独立したファイルに配置しなければならない
3. SerializationExtension と関連型は独立したファイルに配置しなければならない
4. ビルトインシリアライザ（BoolSerializer, StringSerializer 等）は builtin サブモジュールまたは独立ファイルに配置しなければならない
5. エラー型（SerializationError, NotSerializableError, SerializerIdError）は各々独立したファイルに配置しなければならない
6. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件11: spawn サブパッケージの構造改善
**目的:** ランタイム開発者として、アクター生成機能（NameRegistry, SpawnError）を明確に分離し、生成ロジックの拡張を容易にしたい。

#### 受け入れ条件
1. spawn サブパッケージを変更したとき、NameRegistry は独立したファイルに配置しなければならない
2. NameRegistryError は独立したファイルに配置しなければならない
3. SpawnError は独立したファイルに配置しなければならない
4. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件12: extension サブパッケージの構造改善
**目的:** ランタイム開発者として、Extension 機能（Extension trait, ExtensionId, ExtensionInstaller）を pekko の設計に沿って整理し、拡張ポイントの追加を容易にしたい。

#### 受け入れ条件
1. extension サブパッケージを変更したとき、Extension trait は独立したファイルに配置しなければならない
2. ExtensionId は独立したファイルに配置しなければならない
3. ExtensionInstaller は独立したファイルに配置しなければならない
4. ExtensionInstallers（複数インストーラの管理）は独立したファイルに配置しなければならない
5. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件13: lifecycle サブパッケージの構造改善
**目的:** ランタイム開発者として、ライフサイクルイベント（LifecycleEvent, LifecycleStage）を明確に分離し、ライフサイクル監視の拡張を容易にしたい。

#### 受け入れ条件
1. lifecycle サブパッケージを変更したとき、LifecycleEvent は独立したファイルに配置しなければならない
2. LifecycleStage は独立したファイルに配置しなければならない
3. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件14: futures サブパッケージの構造改善
**目的:** ランタイム開発者として、ActorFuture と関連型を明確に分離し、非同期パターンの拡張を容易にしたい。

#### 受け入れ条件
1. futures サブパッケージを変更したとき、ActorFuture は独立したファイルに配置しなければならない
2. ActorFutureListener は独立したファイルに配置しなければならない
3. ActorFutureShared は独立したファイルに配置しなければならない
4. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件15: system サブパッケージの構造改善
**目的:** ランタイム開発者として、ActorSystem と関連コンポーネント（ActorRefProvider, Guardian, RemoteAuthority）を論理的なサブグループに整理し、システム機能の拡張を容易にしたい。

#### 受け入れ条件
1. system サブパッケージを変更したとき、ActorSystem と ActorSystemGeneric は独立したファイル（base.rs）に配置しなければならない
2. ActorRefProvider 関連の型（ActorRefProvider trait, LocalActorRefProvider, インストーラ等）は actor_ref_provider サブモジュールに配置しなければならない
3. Guardian 関連の型（GuardianKind, RootGuardianActor, SystemGuardianActor）は guardian サブモジュールに配置しなければならない
4. RemoteAuthority 関連の型は remote_authority サブモジュールに配置しなければならない
5. SystemState 関連の型は system_state サブモジュールに配置しなければならない
6. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件16: error サブパッケージの構造改善
**目的:** ランタイム開発者として、エラー型（ActorError, SendError）を明確に分離し、エラーハンドリングの拡張を容易にしたい。

#### 受け入れ条件
1. error サブパッケージを変更したとき、ActorError は独立したファイルに配置しなければならない
2. ActorErrorReason は独立したファイルに配置しなければならない
3. SendError は独立したファイルに配置しなければならない
4. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない

---

### 要件17: std サブパッケージの対応する改善
**目的:** ランタイム開発者として、`modules/actor/src/std/` 配下のサブパッケージを core 側の構造改善と同期させ、一貫性を維持したい。

#### 受け入れ条件
1. core 側のサブパッケージ構造を変更したとき、対応する std 側のサブパッケージも同様の構造原則に従わなければならない
2. std/actor を変更したとき、ActorContext と ActorAdapter は各々独立したファイルに配置しなければならない
3. std/typed を変更したとき、core/typed と同様の構造原則に従わなければならない
4. std/dispatch を変更したとき、dispatcher と mailbox は各々独立したサブモジュールまたはファイルに配置しなければならない
5. std/system を変更したとき、ActorSystemBuilder と ActorSystemConfig は各々独立したファイルに配置しなければならない
6. 変更後に `make ci-check` を実行したとき、すべてのテストがパスしなければならない
