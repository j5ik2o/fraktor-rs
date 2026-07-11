# RFC pekko-0010: routing・serialization・patterns（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/routing/`, `serialization/`, `pattern/`（+ `remote/.../serialization/ActorRefResolveCache.scala`） |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 最終照合日 | 2026-07-12 |

## 1. routing

- **PPAT-1.** `RoutingLogic` 実装は **7 種**: RoundRobin / Random / SmallestMailbox / Broadcast / ScatterGatherFirstCompleted / TailChopping / ConsistentHashing（Broadcast も RoutingLogic として実装される）。
- **PPAT-2.** ConsistentHashing は **`SortedMap` ベースのハッシュリング + `virtualNodesFactor`（仮想ノード）**方式であり、ハッシュは MurmurHash、解決はフラット化した配列のバイナリサーチで行う。
- **PPAT-3.** SmallestMailbox のスコア優先順位は「NoRoutee > suspended > サイズ不明・処理中 > サイズ不明・待機 > サイズ既知 > メッセージなし」の昇順選択で、`hasMessages` と `numberOfMessages` の間の race は許容と実装コメントに明記されている（「Race between hasMessages and numberOfMessages here」）。
- **PPAT-4.** `Pool` は routee を子として生成し（既定 supervisor は全 Escalate）、`Group` は既存 actor を `ActorSelection` 経由で束ねる（watch しない）。`Resizer` は `isTimeForResize` / `resize` の 2 メソッドで pool を動的伸縮させるプラグイン点である。

### 1.1 Router の仕組み

- **PPAT-11.** `Router` は `logic` と `routees` を持つ不変値であり、`route()` は `Broadcast(msg)` を unwrap して全 routee へ、それ以外は `logic.select` で 1 体へ送る。一般の `RouterEnvelope` も送信直前に unwrap される。選択結果が `NoRoutee` の場合は deadLetters へ送られる。
- **PPAT-12.** `RoutedActorRef` は生成時に `BalancingDispatcher` との併用を `ConfigurationException` で拒否し、`Pool` かつ resizer ありなら `ResizablePoolCell`、それ以外は `RoutedActorCell` を cell として選ぶ。`RoutedActorCell` は「初期 routee を router actor のスケジュール**前**に生成」し、`ActorRefRoutee` のみ watch する（`ActorSelectionRoutee` は watch しない）。management message（`GetRoutees` / `AddRoutee` / `RemoveRoutee` / `AdjustPoolSize` + auto-received / `Terminated`）は自分の mailbox 経由で処理し、**通常メッセージは mailbox を経由せず直接 routee へ転送**する（送信スレッド上で route する最適化）。`RemoveRoutee` の子停止は「100ms 遅延の `PoisonPill`」による best effort であり、`Terminated` で自動除去 → 全滅かつ `stopRouterWhenAllRouteesRemoved` で router 自身が停止する。router actor の `preRestart` は子を scrap しない（routee は restart を生き延びる）。
- **PPAT-13.** ScatterGatherFirstCompleted は「全 routee へ `ask`（timeout = `within`）し、単一 `Promise` を最初の完了が勝ち取り、結果を `pipeTo(sender)` する」。routees が空の場合は即時に `TimeoutException` を返す。
- **PPAT-14.** TailChopping は「routee をランダムに並べ替え、`interval` 間隔で次の routee へ順次 `ask` を追加発行し、最初の応答を採用、`within` 超過で `AskTimeoutException`」。tail latency 削減のためのバックアップリクエスト技法である。
- **PPAT-15.** `DefaultResizer` は `messagesPerResize` 件ごとに `pressure`（`pressureThreshold = 1`: scheduled かつ mailbox 非空 / `≤ 0`: 処理中 / `> 1`: queue 長 ≥ 閾値、の routee 数）を測り、`rampup = ceil(rampupRate × capacity)`（pressure が capacity に達したとき）と `backoff = floor(−backoffRate × capacity)`（pressure / capacity < backoffThreshold のとき）の和を上下限（既定 1..10）へクランプして増減する。トリガーは `AtomicBoolean` CAS で `Resize` を自 mailbox へ送る非同期方式。
- **PPAT-16.** `OptimalSizeExploringResizer` はサイズ別スループットログを取り、(1) 72h（既定）未活用が続けば `最大活用数 × 0.8` へ縮小、(2) 確率 0.4 で近傍を explore、(3) それ以外は記録上最速のサイズへ半分だけ移動する探索型である。判定は `action-interval`（既定 5s）経過ベース。`resizer` との併用は起動時例外になる（MUST NOT）。
- **PPAT-17.** ConsistentHashing の hash キーは「`hashMapping` → `ConsistentHashable` 実装 → どちらも無ければ警告ログ + `NoRoutee`」の優先順位で決まる。キーが `Array[Byte]` / `String` 以外の場合は serialization でバイト列化される。`virtual-nodes-factor` の既定は 10 で、routees 集合が変わらない限りリングを再構築しない（`AtomicReference` CAS キャッシュ）。
- **PPAT-18.** `Listeners` trait は `Listen` / `Deafen` / `WithListeners` を `listenerManagement` で処理し、`gossip(msg)` が全 listener へ tell する軽量の購読管理である。

## 2. serialization

- **PPAT-5.** バインディングは設定（`serialization-bindings`: クラス → serializer 名）で行い、既定で `allow-java-serialization = off`（Java シリアライズは `DisabledJavaSerializer` に置換）。プリミティブ・ByteString・バイト配列の専用 serializer が既定登録される。
- **PPAT-6.** `Serialization` extension が `serialize` / `deserialize` / `serializerFor` を提供する。ActorRef のパス解決キャッシュは `LruBoundedCache`（capacity 1024 / age threshold 600）を **ThreadLocal に 1 インスタンスずつ**持つ方式で、temp（ask 用一時 actor）パスはキャッシュしない。なおこの実装は `remote` モジュール側にある。

### 2.1 Serializer の契約

- **PPAT-19.** `Serializer` は `identifier: Int`（**0–40 は Pekko 内部予約**）/ `includeManifest` / `toBinary` / `fromBinary`（失敗は `NotSerializableException` を投げうる）を契約とする。ロードは `ExtendedActorSystem` 1 引数コンストラクタを優先し、なければ引数なしコンストラクタを使う。
- **PPAT-20.** `SerializerWithStringManifest` は `includeManifest` を true に固定し、クラス名ではなく文字列 manifest で型を進化互換にする。未知 manifest には `NotSerializableException` を投げることが推奨され、TCP remoting はこれを transient として「ログ + メッセージ破棄」で扱う（それ以外の例外は接続切断＝破損バイト列の兆候として扱う）。
- **PPAT-21.** `AsyncSerializer` は `toBinaryAsync` / `fromBinaryAsync` を提供する persistence journal 向けの契約であり、同期経路から呼ばれた場合は `Await` でブロックしつつ警告ログを出す。
- **PPAT-22.** `serializerFor` の解決順序: クラス別キャッシュ → bindings（サブタイプ優先にソート済み）から `isAssignableFrom` 一致を全抽出 → 一意なら採用 → 曖昧なら Java serializer を除外して再判定 → なお曖昧なら警告を出して先頭を採用する。設定ファイルのほか `SerializationSetup` によるプログラム的登録も可能である。
- **PPAT-23.** `DisabledJavaSerializer` は toBinary / fromBinary の呼び出しごとに **Security マーカー付き警告**を出して `JavaSerializationException` を投げる（identifier は `JavaSerializer` と同一値で、差し替え互換にするため）。`NullSerializer` は identifier 0 固定、`ByteArraySerializer` はバイト列を無コピーで通す。検証設定 `serialize-messages` / `serialize-creators`（既定 off、テスト用）はメッセージ / Props の直列化可能性を実行時検証する。

## 3. patterns

- **PPAT-7.** ask は `/temp` 配下の `PromiseActorRef` で reply を受け、タイムアウトは `AskTimeoutException`。
- **PPAT-8.** `gracefulStop(target, timeout, stopMessage = PoisonPill)` は「watch を張ってから stopMessage を tell し、`Terminated` の受信で成功」という **DeathWatch ベース**の実装である（ポーリングではない）。stopMessage は差し替え可能。
- **PPAT-9.** `retry` は attempts / 固定 delay / 遅延関数 / 指数バックオフのオーバーロードを持ち、`shouldRetry` の既定は「例外時のみ再試行」。
- **PPAT-10.** CircuitBreaker は Closed / Open / HalfOpen の 3 状態で、**`callTimeout` を持つ**（呼び出しがこれを超えると例外でなくても失敗と数える）。HalfOpen は単一プローブ（最初の 1 呼び出しのみ通し、並行呼び出しは fail-fast）。`resetTimeout` は `exponentialBackoffFactor` / `randomFactor` / `maxResetTimeout` により指数伸長できる（既定は無効化相当）。状態は Atomic 継承オブジェクトによる lock-free 実装。

### 3.1 補助パターン

- **PPAT-24.** `pipeTo` は `Future` の成功値をそのまま tell し、失敗を `Status.Failure` として送る（宛先は `to(recipient)` で差し替え可能）。classic 側に「自分へのパイプ」専用 API はなく、`pipeToSelf` は typed 層のみに存在する（pekko-0008 PTY-11）。
- **PPAT-25.** `StatusReply` は `success` / `error` で生成され、`flattenStatusFuture` が `Future[StatusReply[T]]` を `Future[T]` へ平坦化する（`Error` は例外へ、想定外の値は `IllegalArgumentException`）。`askWithStatus` はこの平坦化込みの ask である。
- **PPAT-26.** `after(duration)(value)` は duration が 1 ナノ秒未満かつ有限なら**即時に評価**し（例外は failed Future 化）、それ以外は `scheduleOnce` で遅延する。`timeout(duration)(value)` は期限内に完了しなければ `TimeoutException` で失敗させる（1.2.0 追加）。
- **PPAT-27.** `CircuitBreakersRegistry` は classic extension であり、`pekko.circuit-breaker.default` + ID 別オーバーライド設定から `CircuitBreaker` を生成して `computeIfAbsent` で ID ごとにキャッシュする。

## 4. 参照

- `ConsistentHash.scala:30-132`、`SmallestMailbox.scala:56-93`、`GracefulStopSupport.scala:59-69`、`CircuitBreaker.scala:114-193`、`reference.conf:792-838`
- `Router.scala:58-196`、`RouterConfig.scala:52-453`（Pool / Group / management messages）、`RoutedActorCell.scala:91-216`（watch / 直接 route / RouterActor）、`ScatterGatherFirstCompleted.scala:45-77`、`TailChopping.scala:35-114`、`Resizer.scala:68-321`、`OptimalSizeExploringResizer.scala:99-315`、`ConsistentHashing.scala:66-361`、`Listeners.scala:20-59`、`reference.conf:204-339`（deployment / resizer 既定値）
- `Serializer.scala:36-465`（契約 / manifest / Java 無効化 / ByteArray / Null）、`AsyncSerializer.scala:33-96`、`Serialization.scala:288-483`（解決順序）、`SerializationSetup.scala:23-81`、`reference.conf:129-140, 794-845`
- `PipeToSupport.scala:30-79`、`StatusReply.scala:38-184`、`AskSupport.scala:103-112`、`FutureTimeoutSupport.scala:25-122`、`CircuitBreakersRegistry.scala:35-101`
