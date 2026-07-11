# RFC pekko-0001: Pekko actor アーキテクチャ概観

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/`, `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/` |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 最終照合日 | 2026-07-12 |

## 1. 概要

Pekko の actor 実装は 2 モジュールで構成される（cluster / persistence / stream 系は本シリーズの対象外）。

| モジュール | 規模 | 主なパッケージ |
|-----------|------|---------------|
| `actor`（classic / untyped） | Scala 195 ファイル | `actor`（ActorCell / ActorSystem / FSM / CoordinatedShutdown / Scheduler）、`actor/dungeon`（ActorCell の分割実装）、`dispatch`（Mailbox / Dispatcher / sysmsg）、`event`（EventStream / logging）、`pattern`（ask / CircuitBreaker / retry）、`routing`、`serialization`、`io`、`util` |
| `actor-typed` | Scala 90 ファイル | `Behavior` / `BehaviorInterceptor` / `SupervisorStrategy`、`internal`（interpreter / adapter）、`receptionist`、`pubsub`、`delivery`、`eventstream`、`scaladsl` / `javadsl` |

## 2. 構造上の特徴

- **P-1.** 実行環境は JVM 前提であり、スレッドプール・時刻・スケジューラはすべてライブラリ内蔵である。
- **P-2.** 実行環境の差し替えは**設定（HOCON / `reference.conf`）+ FQCN プラグイン**で行う（`ExecutorServiceConfigurator` / `MailboxType` / dispatcher 設定。詳細は pekko-0009）。
- **P-3.** `ActorCell` は `actor/dungeon/` 配下の trait 群（`Children` / `DeathWatch` / `Dispatch` / `FaultHandling` / `ReceiveTimeout` / `TimerSchedulerImpl`）を mixin する形で責務分割される。
- **P-4.** typed 層（`actor-typed`）は classic ランタイムの上に adapter（`internal/adapter`）で実装される。
- **P-5.** システムメッセージは `dispatch/sysmsg` の `SystemMessage` 連結リスト（LIFO 受信 → 反転で FIFO 復元）で運ばれる。

## 3. 命名・アドレス・selection

- **P-6.** `ActorPath` は `RootActorPath` / `ChildActorPath` の 2 実装であり、`uid` は同名パスの incarnation 識別のみに使われ、等価比較・順序比較には参加しない（`equals` / `compareTo` は名前と親のみを見る）。シリアライズには uid を `#<uid>` フラグメントとして付与する `toSerializationFormat` を使う（MUST。`toString` は uid を落とす）。パス要素名は ASCII 英数字 + 記号 `-_.*$+:@&=,!~';`（`%XX` エンコード可）で、先頭 `$` はシステム生成名に予約されている。
- **P-7.** `Address(protocol, system, host, port)` は `host.isEmpty` ⇔ ローカルスコープ（`hasLocalScope`）であり、`hasGlobalScope` と排他である。ローカルスコープのアドレスを含む参照をリモートへ送ることは安全でないと契約されている。
- **P-8.** `ActorSelection` はパス文字列を `SelectChildName` / `SelectChildPattern`（`*` `?` を含む要素）/ `SelectParent`（`..`）の列にコンパイルし、`deliverSelection` がローカル cell を再帰的に辿ってワイルドカード一致時は複数の子へファンアウトする。一致する子がなければ `EmptyLocalActorRef` へ送られ Dead Letter として観測される（ワイルドカードのファンアウト後を除く）。`Identify(id)` / `ActorIdentity(id, ref)` は auto-received プロトコルであり、`resolveOne()` は Identify の ask 応答が `Some(ref)` でない限り `ActorNotFound` で失敗する。

## 4. spawn 経路と拡張機構

- **P-9.** spawn 経路は三段である: `Props` が `IndirectActorProducer` を遅延生成・キャッシュし（インスタンス化はリフレクション。コンストラクタ署名は Props 構築時に検証）→ `Deployer` が `pekko.actor.deployment` 設定からパスに対する router / dispatcher / mailbox を解決して上書きし → `RepointableActorRef` が仮の `UnstartedCell`（ロック付きキュー）で受信を蓄積し、supervisor の `Supervise` 処理を契機に実 `ActorCell` へ差し替える（`point`。蓄積分は system → user の順でドレイン）。`Supervise` は「新しい子から最初に届く system message」であり、親はここで子の UID を記録して以後の `Failed` 照合（pekko-0004 PSUP-7）に使う。
- **P-10.** Extension は `ExtensionId`（**object identity** で同定。同一インスタンスを使わないと同じ拡張が多重ロードされる）と `registerExtension`（`ConcurrentHashMap` + `CountDownLatch` の putIfAbsent による冪等・競合安全な登録。失敗は Throwable として記録され再試行可能）で管理される。起動時ロードは `pekko.library-extensions`（ロード失敗は起動失敗）と `pekko.extensions`（失敗はログのみでスキップ）の 2 設定キーから行われる。
- **P-11.** `AbstractActor` / `AbstractFSM` / `AbstractProps` / `Patterns` / typed の `javadsl` は Java API 投影であり、意味論は Scala 側と同一である。本シリーズでは個別に扱わない（README のスコープ宣言を参照）。

## 5. パッケージと RFC の対応（カバレッジ宣言）

| 領域 | Pekko | RFC |
|------|-------|-----|
| mailbox / メッセージキュー | `dispatch/Mailbox.scala`, `dispatch/Mailboxes.scala`, `dispatch/sysmsg/` | 0002 |
| dispatcher / executor | `dispatch/{Dispatcher,PinnedDispatcher,BalancingDispatcher,BatchingExecutor}.scala` | 0003 |
| lifecycle / supervision / stash / backoff | `actor/dungeon/{FaultHandling,Children,ChildrenContainer}.scala`, `actor/{FaultHandling,Stash}.scala`, `pattern/BackoffOptions.scala` | 0004 |
| DeathWatch / 終了 / Coordinated Shutdown | `actor/dungeon/DeathWatch.scala`, `actor/CoordinatedShutdown.scala` | 0005 |
| scheduler / receive timeout / timers / FSM | `actor/{LightArrayRevolverScheduler,Timers,FSM}.scala`, `actor/dungeon/ReceiveTimeout.scala` | 0006 |
| EventStream / EventBus / logging | `event/` | 0007 |
| typed 層一式 | `actor-typed`（`internal/` / `scaladsl/` / `pubsub/` / `delivery/` 含む） | 0008 |
| 実行環境の構成・差し替え | dispatcher / executor 設定、`ForkJoinExecutorConfigurator.scala` 等 | 0009 |
| routing / serialization / patterns | `routing/`, `serialization/`, `pattern/` | 0010 |
| 命名 / selection / spawn 経路 / Extension | `actor/{ActorPath,Address,ActorSelection,Props,Deployer,RepointableActorRef,Extension}.scala` | 0001（本 RFC §3–4） |
| io（TCP / UDP / DNS） | `io/` | 0011 |
| `util/`, `japi/`, `javadsl/`, `internal/jfr/` | — | 対象外（README のスコープ宣言） |

## 6. 参照

- `ActorPath.scala:76-465`（要素名文法 / uid / toSerializationFormat）、`Address.scala:38-95`、`ActorSelection.scala:75-271`（コンパイル / deliverSelection / resolveOne）
- `Props.scala:139-236`、`IndirectActorProducer.scala:50-76`、`Deployer.scala:225-276`、`RepointableActorRef.scala:33-132, 222-248`、`ActorSystem.scala:1142-1231`（Extension 登録とロード）
