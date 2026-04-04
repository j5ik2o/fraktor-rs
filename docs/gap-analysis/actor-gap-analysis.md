# actor モジュール ギャップ分析

## 前提

- 比較対象:
  - fraktor-rs: `modules/actor/src/`
  - Pekko: `references/pekko/actor/src/main/scala/org/apache/pekko/actor`
  - Pekko typed: `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed`
- カバレッジ数値は、`private` / `protected` / `internal` を除いた **主要公開契約** を型単位で数えたもの
- classic の Java 継承 DSL (`AbstractActor`, `ReceiveBuilder`, `AbstractActorWithTimers` など) は、Rust ではそのまま移植しにくいため、必要に応じて `n/a` 判定を使う

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 79 |
| fraktor-rs 対応実装数 | 69 |
| カバレッジ（型単位） | 69/79 (87%) |
| ギャップ数 | 10（core/kernel: 3, core/typed: 5, std: 2） |

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 26 | 22 | 85% |
| core / typed ラッパー | 53 | 47 | 89% |
| std / アダプタ | 6 | 6 | 100% |

`std` は Pekko の JVM 依存ランタイム補助（ロギング、スレッド実行器、協調停止、時計/回路遮断器相当）に対応づけている。

## カテゴリ別ギャップ

### classic actor surface ✅ 実装済み 22/26 (85%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `AbstractActor` / `ReceiveBuilder` | `AbstractActor.scala` | `n/a` | - | n/a | Java 継承 DSL。Rust 側は `Actor` trait と関数/クロージャ中心 |
| `AbstractActorWithTimers` | `AbstractActor.scala` | `n/a` | - | n/a | Java mixin API。意味的には `ActorContext::timers()` で代替 |
| `ActorSystem.registerOnTermination` | `ActorSystem.scala` | 未対応 | core/kernel | easy | `when_terminated` 相当はあるが callback 登録 API はない |
| `PoisonPill` / `Kill` の classic 名前付き surface | `ActorRef.scala` | 部分実装 | core/kernel | easy | 内部 `SystemMessage::{PoisonPill,Kill}` と `ActorRef` helper はあるが、Pekko と同じ公開面ではない |

### typed core surface ✅ 実装済み 47/53 (89%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ExtensibleBehavior` | `Behavior.scala` | 未対応 | core/typed | easy | `Behavior` と interceptor はあるが公開型としての段階分離がない |
| `Terminated` 公開 signal 型 | `MessageAndSignals.scala` | 部分実装 | core/typed | easy | 現在は `BehaviorSignal::Terminated(Pid)` のみ |
| `ChildFailed` 公開 signal 型 | `MessageAndSignals.scala` | 部分実装 | core/typed | easy | 現在は `BehaviorSignal::ChildFailed { pid, error }` のみ |
| `BehaviorBuilder` | `javadsl/BehaviorBuilder.scala` | `n/a` | - | n/a | Java DSL 専用 builder。Rust では `Behaviors::*` + closure で代替 |
| `ReceiveBuilder` | `javadsl/ReceiveBuilder.scala` | `n/a` | - | n/a | Java DSL 専用 builder |
| `ActorContext.ask` の classic `Try` 直結表現 | `scaladsl/ActorContext.scala` | 別名で実装済み | core/typed | - | `ask` / `ask_with_status` / `pipe_to_self` は実装済みだが `Try` モデルではない |

### supervision / watch ✅ 実装済み 8/10 (80%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| classic `watch` 失敗時の完全 rollback 契約 | `ActorContext.scala`, `ActorRef.scala` | 部分実装 | core/typed | medium | 今回 `Receptionist` は修正済みだが、watch を使う他の facade でも同様の整理余地がある |
| typed `DeathPactException` 公開型名 | `MessageAndSignals.scala` | 別名で実装済み | core/typed | trivial | `DeathPactError` として提供。機能は実装済みだが名称差あり |

### routing ✅ 実装済み 14/15 (93%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `BalancingPool` typed parity | `Routers.scala` | 部分実装 | core/typed | medium | `BalancingPoolRouterBuilder` はあるが Pekko の surface と完全一致ではない |

### discovery / receptionist ✅ 実装済み 9/9 (100%)

ギャップなし。`Receptionist`, `ServiceKey`, `Register`, `Deregister`, `Subscribe`, `Find`, `Listing`, `Registered`, `Deregistered` は主要契約を概ねカバーしている。

### scheduling / timers ✅ 実装済み 8/8 (100%)

ギャップなし。classic `Scheduler` / `ClassicTimerScheduler` 相当、typed `Scheduler` / `TimerScheduler` 相当は実装済み。

### ref / resolution ✅ 実装済み 6/6 (100%)

ギャップなし。`ActorRef`, `ActorSelection`, `ActorPath`, `ActorRefResolver`, `narrow`, `unsafe_upcast`, `to/from serialization format` まで揃っている。

### delivery / pubsub ✅ 実装済み 8/8 (100%)

ギャップなし。`ProducerController`, `ConsumerController`, `DurableProducerQueue`, `Topic`, `TopicStats`, `WorkPullingProducerController` まで揃っている。

## 内部モジュール構造ギャップ

API ギャップが 87% まで詰まっており、主要カテゴリの致命的欠落は限定的なので、内部構造ギャップも分析対象に含める。

| 構造ギャップ | Pekko側の根拠 | fraktor-rs側の現状 | 推奨アクション | 難易度 | 緊急度 | 備考 |
|-------------|---------------|--------------------|----------------|--------|--------|------|
| receptionist の facade / protocol / runtime 実装がまだ粗く同居 | `actor-typed/receptionist/Receptionist.scala`, `actor-typed/internal/receptionist/ReceptionistMessages.scala` | `modules/actor/src/core/typed/receptionist.rs` が facade + behavior を保持し、protocol 型だけ `receptionist/` 配下に分割 | `core/typed/receptionist/` に behavior 実装も寄せ、公開 facade と内部実装の境界を明確化 | medium | high | 今後 serializer / cluster receptionist 拡張を入れると 1 ファイル集中が重くなる |
| typed delivery に `internal` 層がなく、公開型と制御ロジックが同じ階層に並ぶ | `actor-typed/delivery/*`, `actor-typed/delivery/internal/ProducerControllerImpl.scala` | `modules/actor/src/core/typed/delivery/` 直下に command / settings / behavior / state が並列 | `delivery/internal/` を新設し、controller 実装詳細と公開 DTO を分離 | medium | medium | 現時点で API は揃っているが、再送・永続キュー拡張時に責務が散りやすい |
| classic kernel の public surface が広く、内部補助型まで `pub` に露出しやすい | Pekko classic は package-private / internal API が多い | `modules/actor/src/core/kernel/**` に利用者向けでない `pub` 型が広く存在 | `pub(crate)` へ寄せられるものを継続的に縮小し、入口 facade からの再公開を基準に露出制御 | medium | medium | fraktor は `pub` 露出が多く、型数だけで見ると Pekko を上回る |

## 実装優先度

### Phase 1

| 項目 | 実装先層 | 理由 |
|------|----------|------|
| `ExtensibleBehavior` 相当の公開 surface 追加 | core/typed | 既存 `Behavior` の薄い公開 alias / trait で吸収しやすい |
| `Terminated` 公開 signal wrapper | core/typed | 既存 `BehaviorSignal::Terminated` の薄い wrapper で済む |
| `ChildFailed` 公開 signal wrapper | core/typed | 既存 `BehaviorSignal::ChildFailed` の薄い wrapper で済む |
| `ActorSystem.registerOnTermination` 相当 convenience | core/kernel, core/typed | 既存 `when_terminated` の wrapper 追加で済む |

### Phase 2

| 項目 | 実装先層 | 理由 |
|------|----------|------|
| receptionist 実装の `receptionist/` 配下への再配置 | core/typed | API を壊さず責務を整理できるが、ファイル分割は複数箇所に波及する |
| delivery の `internal` 分離 | core/typed | 既存 controller 群の責務整理が必要 |
| classic control message surface (`PoisonPill` / `Kill`) の Pekko 互換 facade 明確化 | core/kernel | 既存内部機構はあるが、公開面の寄せ方を設計する必要がある |

### Phase 3

| 項目 | 実装先層 | 理由 |
|------|----------|------|
| classic `AbstractActor` / `ReceiveBuilder` 相当の Rust 向け互換 layer | core/kernel or std | Rust では Java 継承 DSL をそのまま移植できず、新規 facade 設計が必要 |
| typed Java DSL (`BehaviorBuilder`, `ReceiveBuilder`) の parity 層 | core/typed | Rust らしさと Pekko 表面互換の折衷が必要 |

### 対象外（n/a）

| 項目 | 理由 |
|------|------|
| `AbstractActorWithTimers` など Java mixin 群 | JVM / Java 継承モデル依存。意味的には既存 timer API でカバー可能 |

## まとめ

- actor モジュールの parity は **主要 typed 契約がかなり埋まっている**。特に routing / receptionist / typed delivery / ref resolver は強い。
- 低コストで前進できるのは、`ExtensibleBehavior`、`Terminated` / `ChildFailed` の公開 wrapper、`registerOnTermination` 相当 convenience の追加。
- 主要ギャップは、Java 継承 DSL 系と、公開 facade に対する内部責務分離の不足。
- 次のボトルネックは API 不足そのものよりも、**receptionist / delivery の内部責務の切り方** に移りつつある。API gap はまだ残るが、構造整理を並行して進めないと以後の parity 実装速度が落ちる。
