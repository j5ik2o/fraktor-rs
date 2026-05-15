## 背景

`RemoteScope` と `RemoteRouterConfig` は actor の配置先をすでに model 化している。また std remote stack には payload serialization、reliable DeathWatch、graceful flush、wire compression が入っている。残る gap は、remote deployment descriptor を target-node actor cell に変換する runtime 境界である。

現状の `Props` は local Rust factory closure を保持できる。この closure は wire-safe ではなく、別 process へ送ってはならない。そのため remote deployment には deployable factory identity と payload contract が必要であり、target node 側でその identity を local registered factory に map する std daemon が必要になる。

## 目標 / 非目標

**目標:**

- child spawn 時に `Deploy { scope: Scope::Remote(..) }` を remote deployment hook へ route する。
- create request、create success、create failure の no_std wire data を追加する。
- std deployment daemon を既存 remoting installer lifecycle の一部として実行する。
- target node で作成された actor の remote `ActorRef` を返し、通常の remote messaging を可能にする。
- unsupported / failed remote deployment を silent local fallback ではなく `SpawnError` / provider error として観測可能にする。

**非目標:**

- 任意の Rust closure や raw `Props` 内部表現を network 越しに送ること。
- Pekko Artery byte-level compatibility。
- cluster membership、placement、load balancing、security policy。
- generic remote code loading。
- daemon がない場合に remote-scoped spawn を local のまま継続する compatibility path。

## 設計判断

### 判断 1: actor-core が dispatch を所有し、std adapter が remote creation を所有する

actor-core は deployment metadata の評価中に `Scope::Remote` を検出し、installed remote deployment hook を呼ぶ。hook は `RemoteCreated`、`UseLocalDeployment`、`Failed` のような outcome を返す。actor-core は TCP、tokio、channel、remote daemon task を知らず、`UseLocalDeployment` の場合だけ通常の local spawn path に戻る。

代替案は、`remote-adaptor-std` が local spawn 後にすべての spawn を検査する方式である。これは local actor creation と remote deployment を race させ、actor-core の spawn contract を曖昧にするため採用しない。

### 判断 2: deployable props は closure serialization ではなく registry identity を使う

remote deployment は deployable props metadata として stable factory id と serialized factory payload を要求する。target daemon は local deployment factory registry でその id を解決し、target node 上で local `Props` を構築する。

代替案は、`Props::factory` や Rust closure capture を serialize する方式である。これは process 間で portable ではなく、安全でもなく、actor-core serialization で検証できない。

### 判断 3: create は同期 spawn surface に合わせた bounded request/response とする

`remote-core` は correlation id を持つ create request、create success、create failure PDU を定義する。既存 actor-core の spawn API は同期的なので、std provider hook 実装は bounded timeout 付きの同期 bridge として matching response または timeout まで待つ。actor-core 側 contract は同期 trait に留め、timeout や待機 primitive は std adapter 側に閉じる。

代替案は、spawn API を async 化する方式である。これは actor-core 全体の public surface と message handling contract を広く変えるため、この change の目的に対して大きすぎる。もう一つの代替案は magic path へ通常の user message を送る方式である。これは protocol failure を actor delivery 内に隠し、create acknowledgement を曖昧にする。

### 判断 4: daemon は remoting lifecycle と一緒に install される

`RemotingExtensionInstaller` が deployment daemon を起動し、remote run task と同じ lifecycle で停止する。daemon は既存の remote event sender、serialization extension、monotonic time source、actor system handle を共有する。

代替案は、public daemon startup API を提供する方式である。これは remote startup からすでに取り除いた lifecycle 分断を再導入し、application が partially installed remoting stack を作れてしまう。

### 判断 5: loopback remote deployment は local deployment として扱う

remote scope address が local node に解決される場合、provider は local deployment に正規化し、TCP 経由の create command を送らない。これは actor ref resolution の既存 provider loopback behavior と一致する。

loopback の場合、std provider hook は local provider で actor を直接作らず、`UseLocalDeployment` outcome を actor-core に返す。これにより local actor creation は actor-core の既存 spawn path に閉じる。

代替案は、loopback も含めて常に remote transport へ create command を送る方式である。これは不要な async failure mode を増やし、local spawn semantics を重複させる。

## リスク / トレードオフ

- deployable factory registry が実 application には弱すぎる可能性がある -> 最初の contract は id + serialized payload + local registry lookup に限定し、明示的かつ testable にする。
- remote create timeout 後に target actor が生き残る可能性がある -> correlation が期限切れになった late success は stale response として観測可能にし、別 change で orphan cleanup policy を追加できるよう failure code を残す。
- bounded synchronous wait が remote event processing を止める可能性がある -> wait は remote run task / TCP reader / response dispatcher 自身では行わず、blocking-safe bridge または configuration failure として扱う。
- spawn error mapping で詳細が失われる可能性がある -> create failure PDU に structured failure code と reason string を含める。
- local child と remote child の lifecycle は同一ではない -> この change では remote stop / suspend / resume protocol を追加せず、該当 `ChildRef` lifecycle command は unsupported failure として観測可能にする。remote child の termination observation は既存 remote DeathWatch path で検証する。

## 移行計画

1. actor-core に deployable props metadata と remote deployment hook API を追加する。
2. `remote-core` に remote deployment wire PDU と codec tests を追加する。
3. std deployment daemon の request handling と provider pending request state を追加する。
4. `Scope::Remote` spawn を provider-driven remote create へ接続する。
5. remote child を作成し、返却 remote ref へ message を送る two-node tests を追加する。
6. 実装後に remote gap analysis を更新する。

rollback は archive 前の active change 削除で行う。pre-release behavior なので compatibility layer は不要である。

## 未解決事項

初回実装で未解決事項はない。authentication と cluster placement は意図的に別 change とする。
