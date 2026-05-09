## 1. 現状固定と設計確認

- [x] 1.1 `UnboundedMessageQueue` / `QueueStateHandle` / `Mailbox::enqueue_envelope` の現行 lock 経路をテストまたはコードコメントで確認する
- [x] 1.2 通常 unbounded queue、bounded queue、deque queue、priority queue の選択経路を確認し、初回 scope が通常 unbounded queue のみであることを固定する
- [x] 1.3 close 後 enqueue rejection、in-flight enqueue vs cleanup、post-drain reschedule の既存テストを確認し、不足分を regression test として追加する

## 2. Lock-free Queue Primitive

- [x] 2.1 mailbox-local module として lock-free MPSC queue primitive を追加する
- [x] 2.2 producer guard を RAII 化し、close protocol 中に in-flight producer count が漏れないようにする
- [x] 2.3 queue-local atomic close protocol を実装し、close 後 enqueue が `Closed` 相当を返すようにする
- [x] 2.4 consumer-side guard を実装し、`MessageQueue` の safe `&self` API から concurrent dequeue されても UB が起きないようにする
- [x] 2.5 raw pointer / node ownership に関する unsafe block を primitive module 内に局所化し、各 unsafe block に SAFETY comment を付ける

## 3. Mailbox Integration

- [x] 3.1 `UnboundedMessageQueue` を lock-free MPSC primitive backing に差し替える
- [x] 3.2 通常 unbounded queue の `enqueue_envelope` path から `put_lock` 取得を外す
- [x] 3.3 lock-backed queue では既存の `put_lock` close serialization を維持する
- [x] 3.4 `clean_up` / `become_closed` 経路で lock-free queue の close-and-drain protocol を呼ぶ
- [x] 3.5 `Mailbox::run` の system-first drain、throughput、deadline、suspend、post-drain reschedule semantics が変わっていないことを確認する

## 4. Verification

- [x] 4.1 lock-free queue の FIFO / exact-once / close rejection / cleanup drain unit tests を追加する
- [x] 4.2 複数 producer と mailbox runner 相当 consumer の stress test を追加する
- [x] 4.3 `loom` model test の配置と dev-dependency/cfg 方針を決め、producer/consumer interleaving を検証する
- [x] 4.4 `miri` で primitive の raw pointer ownership safety を検証できるテストを追加する
- [x] 4.5 `cargo test -p fraktor-actor-core-kernel-rs dispatch::mailbox` を通す
- [ ] 4.6 `cargo clippy -p fraktor-actor-core-kernel-rs --all-targets --all-features -- -D warnings` を通す
- [x] 4.7 OpenSpec validation を通す
