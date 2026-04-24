# actor モジュール ギャップ分析

更新日: 2026-04-24

## 結論

過去調査の「API parity はほぼ 100%」という判断は、限定された actor parity スコープでは妥当である。

今回の初回調査で「ギャップ大」と見えた原因は、Pekko の `actor` / `actor-typed` 配下を raw に広く抽出し、Java / Scala / JVM 固有 API や transport 寄り API まで同じ指標へ混ぜたためである。以降の評価では、Rust で再現すべき actor ランタイム契約にスコープを限定する。

## 比較スコープ定義

### 対象に含めるもの

fraktor-rs の actor parity 対象は、Pekko の実装を参考にしつつ、Rust の actor ランタイムとして意味を持つ公開契約に限定する。

| 分類 | Pekko 側の主な参照 | fraktor-rs 側の対応 |
|------|--------------------|---------------------|
| classic / untyped actor core | `references/pekko/actor/` の actor core | `modules/actor-core/src/core/kernel/` |
| typed actor core | `references/pekko/actor-typed/` の typed API | `modules/actor-core/src/core/typed/` |
| supervision / lifecycle | fault handling, DeathWatch, signals | `modules/actor-core/src/core/kernel/`, `modules/actor-core/src/core/typed/` |
| dispatch / mailbox | dispatcher, executor abstraction, mailbox contract | `modules/actor-core/src/core/kernel/`, `modules/actor-adaptor-std/src/std/` |
| routing | classic / typed routing semantics | `modules/actor-core/src/core/kernel/`, `modules/actor-core/src/core/typed/` |
| event / logging | event stream, dead letters, logging contract | `modules/actor-core/src/core/kernel/`, `modules/actor-adaptor-std/src/std/` |
| pattern | ask, pipe, retry, graceful stop, circuit breaker | `modules/actor-core/src/core/kernel/`, `modules/actor-core/src/core/typed/` |
| receptionist / discovery | typed receptionist, service key, listing | `modules/actor-core/src/core/typed/` |
| delivery / pubsub | typed reliable delivery, local topic | `modules/actor-core/src/core/typed/` |
| serialization contract | serializer trait, registry, manifest | `modules/actor-core/src/core/kernel/` |
| coordinated shutdown | phase, task, termination contract | `modules/actor-core/src/core/kernel/` |
| std adaptor | tokio / tracing / executor / scheduler adapter | `modules/actor-adaptor-std/src/std/` |

### 対象外にするもの

次は actor parity の対象外とする。理由は、Rust で同じ API を再現しても価値が低い、または JVM / Scala / Java 実装都合に強く依存するためである。

| 対象外 | 理由 |
|--------|------|
| Java DSL: `AbstractActor`, `ReceiveBuilder`, `BehaviorBuilder`, `javadsl/*` | Java 継承 DSL / builder DSL であり、Rust では `Actor` trait と closure / typed `Behavior` で表現する |
| `japi/*` | Java interop 専用 |
| Scala implicit / package ops | Scala 構文拡張であり、Rust API として同型にする必要がない |
| JVM reflection / classloader: `DynamicAccess`, `ReflectiveDynamicAccess`, `ClassLoaderObjectInputStream` | JVM 固有 |
| HOCON dynamic loading / configurator facade | JVM 設定ロード方式に依存。Rust では builder / typed config で扱う |
| Java serialization: `JavaSerializer`, `DisabledJavaSerializer` | JVM Java serialization 固有 |
| JFR / flight recorder events | JVM observability 固有。Rust では tracing / metrics 側で扱う |
| deprecated classic remoting / Netty / Aeron 固有実装 | 廃止または実装技術固有 |
| Pekko IO / TCP / UDP / DNS | actor core ではなく transport / network adapter 責務として別スコープで扱う |
| Pekko util 全体互換 | actor runtime に必要な `ByteString` 等のみ対象。汎用 util ライブラリ互換は対象外 |

## サマリー

| 指標 | 値 |
|------|-----|
| 分析スコープ | Rust actor runtime parity |
| parity 対象 API 数 | 101 |
| fraktor-rs 対応数 | 101 |
| 公開 API カバレッジ | 101/101 (100%) |
| 公開 API ギャップ | 0 |
| high ギャップ | 0 |
| medium ギャップ | 1 |
| medium ギャップ内容 | remote / cluster 連携後の `AddressTerminated` DeathWatch 統合 |
| スタブ検出 | `todo!()` / `unimplemented!()` は actor-core / actor-adaptor-std で 0 件 |

## 層別カバレッジ

| 層 | parity 対象数 | fraktor-rs 対応数 | カバレッジ |
|----|---------------|------------------|-----------|
| core / untyped kernel | 39 | 39 | 100% |
| core / typed wrapper | 56 | 56 | 100% |
| std / adaptor | 6 | 6 | 100% |
| 合計 | 101 | 101 | 100% |

## カテゴリ別カバレッジ

| カテゴリ | 判定 | 備考 |
|----------|------|------|
| classic actor core | 実装済み | actor, context, ref, path, selection, props, system message, stash, timer |
| supervision / fault handling | 実装済み | supervisor strategy, directive, restart policy, backoff supervisor |
| typed core surface | 実装済み | typed ref, typed system, behavior, signal, interceptor, context |
| dispatch / mailbox | 実装済み | dispatcher abstraction, mailbox contract, bounded / priority / control-aware mailbox |
| event / logging | 実装済み | event stream, dead letter, logging adapter, std tracing integration |
| pattern | 実装済み | ask, pipe, retry, graceful stop, circuit breaker |
| classic routing | 実装済み | router, routee, routing logic, pool / group equivalent |
| typed routing | 実装済み | pool, group, scatter-gather, tail-chopping, balancing |
| receptionist / discovery | 実装済み | service key, receptionist command, listing, local registration |
| scheduling / timers | 実装済み | scheduler, timer scheduler, receive timeout |
| ref / resolution | 実装済み | actor path, actor selection, identify / identity |
| delivery / pubsub | 実装済み | producer / consumer controller, durable producer queue, topic |
| serialization | 実装済み | serializer trait, manifest, registry, transport information |
| extension | 実装済み | extension id, setup, registry |
| coordinated shutdown | 実装済み | phase, task, reason, termination |
| std adaptor | 実装済み | tokio executor, threaded executor, scheduler, tracing subscriber |

## 残存ギャップ

### AC-M4b: DeathWatch の remote `AddressTerminated` 統合

| 項目 | 内容 |
|------|------|
| 種別 | 内部セマンティクスギャップ |
| Pekko 側の根拠 | remote address termination を DeathWatch に流し、監視対象 actor へ一度だけ termination を通知する |
| fraktor-rs 側の現状 | local DeathWatch は実装済み。remote / cluster の address termination 連携は後続モジュール依存 |
| 実装先 | core + remote / cluster integration |
| 難易度 | medium |
| 優先度 | remote / cluster の基盤確定後 |

この項目は公開 API ギャップではない。remote / cluster が actor runtime に接続された段階で、内部イベント経路として実装する。

### classic kernel public surface の整理

| 項目 | 内容 |
|------|------|
| 種別 | 内部構造ギャップ |
| 現状 | classic kernel の補助型が public surface に広く露出している |
| 問題 | 外部 API と内部実装詳細の境界がやや曖昧 |
| 推奨 | 外部契約として必要な型と `pub(crate)` に落とせる補助型を分離する |
| 難易度 | medium |
| 優先度 | API 追加よりも構造整理タスクとして扱う |

この項目も Pekko parity の公開 API 不足ではない。今後の保守性を上げるための構造改善候補である。

## raw 抽出結果の扱い

今回の初回調査では、参考値として次の raw 抽出を行った。

| 指標 | 値 |
|------|-----|
| Pekko `actor` + `actor-typed` raw 公開型数 | 756 unique |
| fraktor-rs actor raw 公開型数 | 472 unique |
| 直接同名カバレッジ | 134/756 (17.7%) |

この数値は parity 指標として使わない。理由は、次のような対象外 API を含むためである。

- Java DSL / javadsl / japi
- Scala implicit / package ops
- JVM reflection / classloader
- HOCON dynamic loading
- Java serialization
- JFR events
- deprecated classic remoting
- Pekko IO / TCP / UDP / DNS
- actor runtime に不要な Pekko util 全体互換

raw 抽出は「Pekko ディレクトリ内に存在する名前の棚卸し」としては有用だが、「Rust actor runtime parity の達成率」としては不適切である。

## 実装優先度

### Phase 1: trivial / easy

公開 API parity 対象では該当なし。

### Phase 2: medium

- remote / cluster 基盤確定後に `AddressTerminated` を DeathWatch へ統合する（core + remote / cluster）
- classic kernel の public surface を整理し、内部補助型を `pub(crate)` 化する（core/kernel）

### Phase 3: hard

公開 API parity 対象では該当なし。

### 対象外（n/a）

- Java DSL / javadsl / japi
- Scala implicit / package ops
- JVM reflection / classloader
- HOCON dynamic loading
- Java serialization
- JFR / flight recorder
- deprecated classic remoting
- Pekko IO / TCP / UDP / DNS
- actor runtime に不要な Pekko util 全体互換

## まとめ

actor モジュールの Rust actor runtime parity は、公開 API レベルでは 101/101 で達成済みと評価する。残っている主な作業は、公開 API 追加ではなく、remote / cluster 連携後の DeathWatch 内部セマンティクスと classic kernel の public surface 整理である。

今後の調査では、Pekko の raw ディレクトリ抽出結果をそのまま parity 分母にしない。まず「Rust で再現すべき公開契約」と「JVM / Scala / Java / transport 固有で対象外にする契約」を固定し、そのスコープ内でギャップを評価する。
