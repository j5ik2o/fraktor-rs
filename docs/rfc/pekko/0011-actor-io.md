# RFC pekko-0011: io サブシステム（TCP / UDP / DNS）（Pekko）

| 項目 | 内容 |
|------|------|
| Status | As-built (reference) |
| 対象コード | `references/pekko/actor/src/main/scala/org/apache/pekko/io/`（`IO.scala`, `Tcp.scala`, `TcpManager.scala`, `TcpListener.scala`, `TcpConnection.scala`, `SelectionHandler.scala`, `Udp.scala`, `UdpConnected.scala`, `Dns.scala`, `dns/`） |
| 照合コミット | `references/pekko` @ `2dc8960074` |
| 最終照合日 | 2026-07-12 |

本 RFC は番号対応（0001–0010）の枠外であり、「Pekko が I/O を actor プロトコルとしてどう表現しているか」の参照資料として収載する（README のスコープ宣言を参照）。深さは概説レベルに留める。

## 1. 規範仕様

### 1.1 Extension → manager actor パターン

- **PIO-1.** 入口は `IO(Tcp)` / `IO(Udp)` / `IO(UdpConnected)` / `IO(Dns)` であり、いずれも Extension が保持する **manager actor**（`IO-TCP` / `IO-UDP-FF` / `IO-DNS` 等の system actor）への `ActorRef` を返す。以後の操作はすべてこの manager への通常メッセージで行われる（専用 API はなく、actor プロトコルそのものが公開面である）。

### 1.2 Tcp プロトコル

- **PIO-2.** コマンドは `Connect` / `Bind` / `Register(handler, keepOpenOnPeerClosed, useResumeWriting)` / `Unbind` / `Write` 系 / `ResumeWriting` / `SuspendReading` / `ResumeReading` / `ResumeAccepting`、イベントは `Connected` / `Bound` / `Received(data)` / `CommandFailed(cmd)` / `WritingResumed` / `ConnectionClosed` 系（`Closed` / `ConfirmedClosed` / `Aborted` / `PeerClosed` / `ErrorClosed`）である。
- **PIO-3.** 書き込みは ack/nack ベースのフロー制御を持つ: `Write(data, ack)` の `ack` が `NoAck` でなければ成功時に ack トークンがそのまま返信され、受理できない場合は `CommandFailed`（NACK）が返る。`Register(useResumeWriting = true)` の場合、NACK 後は `ResumeWriting` を送るまで後続の書き込みがすべて拒否される（MUST）。
- **PIO-4.** 読み取りは pull mode を選択できる: `pullMode = true` で `Connect` / `Bind` すると明示的な `ResumeReading` まで読まない。`SuspendReading` / `ResumeReading` で TCP の流量制御を送信側へ伝播させる。`ResumeReading` は `DeadLetterSuppression` 実装である（接続終了と同時に届いて dead letter になりやすいため）。
- **PIO-5.** close の 3 形態は意味が異なる: `Close`（フラッシュ後に通常クローズ → `Closed`）/ `ConfirmedClose`（フラッシュ後に half-close し相手のクローズを待つ → `ConfirmedClosed`）/ `Abort`（フラッシュせず RST 送出 → `Aborted`）。

### 1.3 役割分担（NIO セレクタ層）

- **PIO-6.** `TcpManager` はコマンドを受けて worker（`TcpOutgoingConnection` / `TcpListener`）の Props を `SelectionHandler` プール（`RandomPool(nr-of-selectors)`）へ委譲する facade であり、自身は NIO 操作をしない。`SelectionHandler` が `ChannelRegistry` としてセレクタ管理を隠蔽し、各 worker は `ChannelRegistration`（interest の付け外しと cancelAndClose）を介してのみセレクタへ触れる。
- **PIO-7.** 接続・リスナー actor の親戦略は「全エラーで子を停止」であり、`DeathPactException` は debug ログのみで扱われる（handler 終了 = 接続クローズという運用が正常系）。

### 1.4 Udp / UdpConnected / Dns

- **PIO-8.** `Udp` はソケットを connect せず毎回 `Send(payload, target)` で宛先を指定するコネクションレス、`UdpConnected` はソケットレベルの connect を行い相手を固定する（`Send(payload)` に宛先がない）。両者は別 Extension である。
- **PIO-9.** `Dns` は `DnsProtocol.Resolve(name, requestType)` → `Resolved` の actor プロトコルで、`Resolve` は `ConsistentHashable`（`consistentHashKey = name`）を実装しリゾルバ群への一貫ハッシュルーティングに対応する。キャッシュポリシーは `Never` / `Forever` / `Ttl` の 3 種で、`SimpleDnsCache` は `AtomicReference` CAS + 期限順序付き cleanup を持つ。リゾルバ実装は `DnsProvider`（既定 `inet-address` = JDK ブロッキング解決、`async-dns` = ネイティブ非同期実装）で差し替え可能である。

### 1.5 主要既定値

- **PIO-10.** `pekko.io.tcp`: `nr-of-selectors = 1` / `max-channels = 256000` / `register-timeout = 5s` / `direct-buffer-size = 128 KiB` / `batch-accept-limit = 10`。`pekko.io.dns`: `resolver = "inet-address"` / `cache-cleanup-interval = 120s`、`async-dns` は `positive-ttl = forever` / `negative-ttl = never` / `resolve-timeout = 5s`。

## 2. 不変条件

- **INV-PIO-1**: `useResumeWriting` 有効時、`CommandFailed` の後に `ResumeWriting` なしで書き込みが受理されることはない（PIO-3）。
- **INV-PIO-2**: pull mode の接続がユーザーの明示的な read 許可なしにデータを読み進めることはない（PIO-4）。
- **INV-PIO-3**: worker actor がセレクタへ直接触れることはない（`ChannelRegistration` 経由のみ、PIO-6）。

## 3. 参照

- `IO.scala:26-38`、`Tcp.scala:133-649`（プロトコル定義と `TcpExt`）、`TcpManager.scala:57-71`、`TcpListener.scala:60-152`、`TcpConnection.scala:42-587`、`SelectionHandler.scala:56-149`
- `Udp.scala:30-250`、`UdpConnected.scala:28-117`、`Dns.scala:41-168`、`DnsProtocol.scala:38-127`、`dns/CachePolicy.scala:22-76`、`SimpleDnsCache.scala:35-142`、`DnsProvider.scala:20-46`
- `reference.conf:934-1219`（`pekko.io.*` 既定値）
