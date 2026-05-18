# remote 内部構造ギャップ解消の OpenSpec change 計画

## 背景

`docs/gap-analysis/remote-gap-analysis.md` の内部モジュール構造ギャップは、公開 API 追加だけでは解消できない。残る gap は `remote-core` / `remote-adaptor-std` / `actor-core-kernel` をまたぐため、1 つの巨大 change ではなく、観測可能な機能ごとに OpenSpec change へ分ける。

## 方針

- `remote-core` の no_std state / wire contract を先に固め、`remote-adaptor-std` の timer / task / TCP driver は後から接続する。
- 各 change は、完了時点で単体の機能として動作することを必須にする。純粋 state / wire 型 / task skeleton だけの change は作らない。
- behavior が actor-core に見えるものは OpenSpec spec delta を持たせる。
- Pekko byte compatibility は狙わず、fraktor-native wire 上で責務とセマンティクスを揃える。
- compression / flush / deployment は土台の serialization と remote system behavior が入るまで着手しない。

## 推奨 change

| 順序 | change 名 | 目的 | 主な対象 | 完了条件 |
|------|-----------|------|----------|----------|
| 1 | `remote-payload-serialization` | 任意 actor message を TCP remote で送受信できるようにする | `actor-core-kernel` serialization、`remote-core` wire envelope metadata、`remote-adaptor-std` TCP lane | `Bytes` / `Vec<u8>` 限定を外し、登録済み serializer の message が two-node TCP test で round trip する |
| 2 | `remote-reliable-deathwatch` | remote watch / unwatch / termination notification を実用化する | `remote-core` ACK/NACK redelivery state + watcher state、`remote-adaptor-std` watcher task / retry driver、`actor-core-kernel` DeathWatch | remote actor の終了が watcher に通知され、watch system message は ACK/NACK resend により一時的な欠落から回復する |
| 3 | `remote-graceful-flush` | shutdown flush と DeathWatch 前 flush を実用化する | `remote-core` flush PDU / control、`remote-adaptor-std` wait driver、DeathWatch integration | shutdown 前に flush ack / timeout を待ち、DeathWatch notification は flush 成功または timeout 後に発行される |
| 4 | `remote-wire-compression` | actor-ref / serializer manifest compression を実際の wire lane に適用する | `remote-core` compression table state、`remote-adaptor-std` advertisement timer、TCP codec | table advertisement / ack / hit counting があり、manifest / actor ref を圧縮した frame が encode / decode される |
| 5 | `remote-deployment-daemon` | `RemoteScope` を remote child actor 作成へ接続する | `actor-core-kernel` deployer、`remote-adaptor-std` provider / daemon、serialization | remote create command により remote node 上で actor が起動し、返された remote ref へ message を送れる |

## 最初に作る change

最初は `remote-payload-serialization` がよい。理由は、単体で明確な機能になるため。現状の TCP remote は `Bytes` / `Vec<u8>` だけを送れるが、この change で登録済み serializer を持つ任意 message を送れるようにする。

1. wire envelope に serializer id / manifest / payload metadata を持たせる。
2. outbound lane で `SerializationExtension` または `SerializationRegistry` を使って `AnyMessage` を `SerializedMessage` に変換する。
3. inbound lane で `SerializedMessage` を `AnyMessage` に戻して既存 dispatch に渡す。
4. two-node TCP test で `String` など登録済み serializer の message round trip を確認する。

`remote-system-message-redelivery` は独立 change にしない。state machine だけでは利用者から見える機能にならないため、`remote-reliable-deathwatch` の中で remote DeathWatch の信頼性として実装する。

## まだ切らない change

- `remote-wire-compression`: `remote-payload-serialization` が先。serializer manifest の流路がないと table を持っても適用先がない。
- `remote-graceful-flush`: `remote-reliable-deathwatch` が先。DeathWatch 前 flush を含めて初めて機能として意味がある。
- `remote-deployment-daemon`: `remote-payload-serialization` と DeathWatch 境界が先。Props / deploy command / security guard が絡むため最後寄りにする。

## 検証方針

- 各 change は対象 crate の unit test を先に通す。
- TCP lane や actor-core 連携を含む change は `remote-adaptor-std` の two-node integration test を追加する。
- 最終段階で `./scripts/ci-check.sh ai all` を実行する。
