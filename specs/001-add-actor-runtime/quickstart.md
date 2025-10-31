# Quickstart: セルアクター no_std ランタイム初期版

## 1. 前提

- Rust 1.81 (stable) と nightly ツールチェーン (`rustup toolchain install nightly`).
- `probe-rs` または `cargo-embed` によるターゲットボード書き込み環境。
- 対象ボード: RP2040 (thumbv6m-none-eabi) / RP235x (thumbv8m.main-none-eabihf)。
- `makers` ワークフローと `./scripts/ci-check.sh` が実行可能であること。

## 2. ビルド構成

```bash
# ホスト向け（デバッグ用、std フィーチャは無効）
cargo build --package actor-core --no-default-features

# no_std 組込みターゲット
target=thumbv8m.main-none-eabihf
cargo build --package actor-core --target $target --no-default-features
```

## 3. サンプル実行（Ping/Pong）

1. `examples/ping_pong_no_std` を `actor-core` に追加し、`AnyMessage::new(Ping)` → `downcast_ref::<Ping>()` の往復を確認します。
2. ホスト: `cargo run --example ping_pong_no_std --no-default-features`（actor-core では `std` フィーチャを有効化しない）。
3. 組込み: `cargo embed --example ping_pong_no_std --target $target`。UART ログに `PING -> PONG` が 1,000 回出力され、1 秒以内に完了すること。

## 4. 監督戦略と Deadletter の確認

- `examples/supervision_std` を実行し、`ActorError::recoverable` を返すと Supervisor が再起動を行い、EventStream 経由でライフサイクルログが出力されることを確認する。  
- `examples/deadletter_std` を実行し、Mailbox を `Suspend` した状態で `tell` すると Deadletter/LogEvent が発生することを確認する。サンプル内では `send_or_log` `suspend_or_log` といった薄いヘルパーで `Result<(), SendError>` を扱っている。  
- `panic!()` を挿入した場合はランタイムが停止を記録し、外部ウォッチドッグ（例: RP2040 の watchdog リセット）がシステムを再起動する設計とする。

## 5. ガーディアンアクターでのエントリポイント

```rust
let guardian_props = Props::new(|ctx| GuardianActor::new(ctx));
let system = ActorSystem::new(&guardian_props);
system
  .user_guardian_ref()
  .tell(AnyMessage::new(Start))
  .expect("bootstrap");

let termination = system.when_terminated();
system.terminate().expect("terminate");
while !termination.is_ready() {
  core::hint::spin_loop();
}
```

- guardian が `spawn_child` を利用してアプリケーションの子アクターを構築する。トップレベルのアクター生成はガーディアン（または子アクター）からの `spawn_child` のみ許可し、外部コードは `user_guardian_ref()` に `Start` メッセージを送ることでアプリケーションを起動する。
- 名前付きアクター: `ctx.spawn_child(props.with_name("worker"))` で同親スコープ内の一意性を確認。名前未指定では `anon-{pid}` が割り当てられる。
- リクエスト/リプライ: メッセージに `reply_to: ActorRef` を含め、`sender()` を使用しない。Pong は `reply_to.tell(...)` で返送する。
- ミドルウェアチェーン: `system.with_middleware(logging_middleware)` のように差し込めるポイントがあり、初期状態では空チェーンで動作することを確認。
- Mailbox 戦略: `Props::with_mailbox_strategy` で Bounded/Unbounded を切り替え、Bounded 時は容量（例:64）とポリシーを設定、Unbounded 時は EventStream の警告ログを監視。`throughput_limit` を `Props::with_throughput(300)` などで指定し、上限到達で処理が次ターンに繰り越されることを確認。
- テスト時は別の guardian Props を渡してシナリオを切り替えられる。

## 6. EventStream / Deadletter 購読

```rust
let logger_writer: ArcShared<dyn LoggerWriter> = ArcShared::new(MyWriter);
let logger = ArcShared::new(LoggerSubscriber::new(LogLevel::Info, logger_writer));
let _log_subscription = system.subscribe_event_stream(logger);

struct LifecyclePrinter;

impl EventStreamSubscriber for LifecyclePrinter {
    fn on_event(&self, event: &EventStreamEvent) {
        if let EventStreamEvent::Lifecycle(lifecycle) = event {
            log::info!("actor {:?} -> {:?}", lifecycle.pid(), lifecycle.stage());
        }
    }
}

let lifecycle = ArcShared::new(LifecyclePrinter);
let _lifecycle_subscription = system.subscribe_event_stream(lifecycle);

let snapshot = system.deadletters();
```

- Deadletter の既定保持件数は 512 件。組込み環境では必要に応じて減らし、警告閾値を LoggerSubscriber などで監視する。
- EventStream で `LifecycleEvent`・`LogEvent`・`Deadletter` が受信できることをテストする。

## 7. ヒープ確保計測

1. `alloc::GlobalAlloc` をラップしたカウンタを有効化（feature `alloc-metrics`）。
2. サンプル実行後、`heap_allocations_per_sec` が 5 未満であることを出力。
3. 増加した場合は Mailbox capacity や replay バッチサイズを調整。

## 8. CI & Lint

```bash
# 全タスクを完了したらまとめて実行
./scripts/ci-check.sh all
makers ci-check -- dylint
```

- red テスト（ユーザーストーリー別）を先に実装し、green でコミット。CI スクリプトは最終確認時に一括実行する。
- `panic!` を伴うテストは `thumbv8m` ターゲットで `panic=abort` を指定。

## 9. 運用ノート

- panic 非介入: ランタイムは再起動せず、外部ウォッチドッグまたはシステムサービスが責務を負う。  
- Deadletter / EventStream による監視を運用ダッシュボード（例: RTT 経由）へ出力し、Logger 購読者を通じて `LogEvent` を UART/RTT またはホストログに転送する。  
- 将来の Typed レイヤーは AnyMessage 上に別レイヤーとして構築予定で、現フェーズの API は未型付けを前提とする。
- `tell` は `Result<(), SendError>` を返すため、`send_or_log` のような薄いヘルパーをアプリ側で用意し、サスペンドや容量超過を即座に検知・ロギングする。

> **Reply-to パターンについて**
> このランタイムは Classic の `sender()` を提供しないため、返信が必要な場合はメッセージ起点で `reply_to: ActorRef` を明示的に渡す必要があります。例として、Guardian が自分自身を起点に Ping/Pong を起動する場合:
>
> ```rust
> let start_ping = StartPing { target: pong, reply_to: ctx.self_ref(), count: 3 };
> ping.tell(AnyMessage::new(start_ping))?;
> ```
>
> 受信側の Pong では `reply_to.tell(AnyMessage::new(PongReply { ... }))` のように、受け取った `reply_to` を利用して応答を返します。`ActorContext::reply()` は拡張／ミドルウェア向けの補助メソッドとして残っていますが、アプリケーションレベルでは payload に `reply_to` を含めるスタイルが基本になります。

> **ActorSystem の停止**
> Typed スタイルと同様に、ユーザガーディアンが `ctx.stop(ctx.self_ref())` を呼び出すまで ActorSystem は終了しません。アプリケーションを終了させる場合は、ガーディアンが自ら停止し、それに追随して子アクターやリソースが片付くよう設計してください。`ctx.stop_self()`（または `ctx.stop(ctx.self_ref())`）を呼ぶと保持している子アクターへも `SystemMessage::Stop` が自動伝播し、順次停止処理が進みます。システム終了を待つには `ActorSystem::when_terminated()` で Future を取得し、同期環境では `run_until_terminated()` などのブロッキング API、非同期環境では `await` を利用します。

> **ActorSystem の明示的停止**
> アプリケーション側で明示的にシステムを終了したい場合は、`system.terminate()` を呼び出して内部 `SystemMessage::Stop` をガーディアンに送ります。その後 `run_until_terminated()` でブロックするか、`when_terminated().listener()` を await して終了まで待機してください。
