# Quickstart: セルアクター no_std ランタイム初期版

## 1. 前提

- Rust 1.81 (stable) と nightly ツールチェーン (`rustup toolchain install nightly`).
- `probe-rs` または `cargo-embed` によるターゲットボード書き込み環境。
- 対象ボード: RP2040 (thumbv6m-none-eabi) / RP235x (thumbv8m.main-none-eabihf)。
- `makers` ワークフローと `./scripts/ci-check.sh` が実行可能であること。

## 2. ビルド構成

```bash
# ホスト向け（デバッグ用、std フィーチャのみテストで有効）
cargo build --package actor-core --features std

# no_std 組込みターゲット
target=thumbv8m.main-none-eabihf
cargo build --package actor-core --target $target --no-default-features
```

## 3. サンプル実行（Ping/Pong）

1. `examples/ping_pong_no_std` を `actor-core` に追加し、`AnyMessage::new(Ping)` → `downcast_ref::<Ping>()` の往復を確認します。
2. ホスト: `cargo run --example ping_pong_no_std --features std`（テスト用に std を許可）。
3. 組込み: `cargo embed --example ping_pong_no_std --target $target`。UART ログに `PING -> PONG` が 1,000 回出力され、1 秒以内に完了すること。

## 4. Supervisor 戦略の確認

- `examples/supervisor_restart` を実行し、`Err(ActorError::Recoverable)` で再起動カウンタが増えることを確認。  
- `Err(ActorError::Fatal)` を返すとアクターが停止し、Deadletter + EventStream に記録されること。  
- `panic!()` を挿入した場合はランタイムが停止を記録し、外部ウォッチドッグ（例: RP2040 の watchdog リセット）がシステムを再起動する設計とする。

## 5. EventStream / Deadletter 購読

```rust
let subscription = system.event_stream().subscribe(|event| {
    // ログやテレメトリに流用
});

let deadletter_rx = system.deadletter().subscribe();
```

- Deadletter に蓄積されたメッセージ数が常に 10 未満であることを確認。
- EventStream で `ActorLifecycle::Transition` が受信できることをテスト。

## 6. ヒープ確保計測

1. `alloc::GlobalAlloc` をラップしたカウンタを有効化（feature `alloc-metrics`）。
2. サンプル実行後、`heap_allocations_per_sec` が 5 未満であることを出力。
3. 増加した場合は Mailbox capacity や replay バッチサイズを調整。

## 7. CI & Lint

```bash
./scripts/ci-check.sh all
makers ci-check -- dylint
```

- red テスト（ユーザーストーリー別）を先に実装し、green でコミット。
- `panic!` を伴うテストは `thumbv8m` ターゲットで `panic=abort` を指定。

## 8. 運用ノート

- panic 非介入: ランタイムは再起動せず、外部ウォッチドッグまたはシステムサービスが責務を負う。  
- Deadletter / EventStream による監視を運用ダッシュボード（例: RTT 経由）へ出力。  
- 将来の Typed レイヤーは AnyMessage 上に別レイヤーとして構築予定で、現フェーズの API は未型付けを前提とする。
