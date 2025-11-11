# ルート/システムガーディアン設計

## 目的
- ActorSystemGeneric に Pekko と同等のルート→システム→ユーザ階層を導入し、監督戦略と終了シーケンスを明確化する。
- `/system` 配下の内部アクターと `/user` 配下のユーザアクターを論理的に分離し、CoordinatedShutdown やクラスタ機能の基盤を整える。

- Pekko classic `LocalActorRefProvider` (`references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorRefProvider.scala`)
- Pekko typed `ActorSystemAdapter` および `GuardianStartupBehavior`
- （参考）protoactor-go には固定の `/user` `/system` 階層は存在しないため、本設計では Pekko を第一参照とする

## アクター階層
```
/
├── user (ユーザガーディアン)
├── system (システムガーディアン)
├── temp (一時アクターコンテナ)
├── deadLetters
└── <extra-top-level> (拡張やテスト用に予約)
```

- ルートガーディアンは内部専用で、DeathWatch／予約パス解決／extra names の保持のみ行う。
- `/user` と `/system` はルート生成直後に初期化し、他のトップレベル子は初期化前に登録する。
- `/temp` は VirtualPathContainer 相当のノードとして実装し、`register_temp_actor` / `unregister_temp_actor` API で ask などの一時アクター参照を管理する。

## 初期化フロー
Rust では Pekko の `lazy val` 相当が存在しないため、ガーディアンを明示的な順序で初期化する。

1. **root_guardian を構築**: Props/dispatcher/mailbox を確定して `Arc<ActorCell>` を生成するが、この時点では `start()` を呼ばずに `register_extra_top_level` の受付ウィンドウを確保する。`get_parent()` は常に `None`。
2. **user_guardian を生成**: `root_guardian.reserve_child("user")` の後、ユーザ設定可能な Props／監督戦略を適用して生成。`ActorSystemConfig` で Strategy が未指定ならデフォルトを使用。
3. **system_guardian を生成**: `root_guardian.reserve_child("system")` の後、`create_system_guardian(root_guardian.clone(), user_guardian.clone())` を呼び出して user への参照を渡す。これにより Pekko の `SystemGuardian(guardian: ActorRef)` パターンを明示的に再現する。
4. **追加トップレベル登録ウィンドウ**: 拡張やテストコードはこの段階で `register_extra_top_level` を呼び出し、`AlreadyStarted` エラーなしで `/metrics` などを登録できる。必要な登録が完了したタイミングで `root_guardian.start()` を呼ぶ。
5. **DeathWatch を設定し API を接続**: `system_guardian.watch(user_guardian)` → `/user` 終了時に TerminationHook フェーズ開始。`root_guardian.watch(system_guardian)` → `/system` 終了時に ActorSystem 停止フラグを立てる。DeathWatch 設定と extra top-level 登録が完了したら `root_guardian.start()` を実行し、`ActorSystem::tell` や `spawn` を `/user` ガーディアンへのデリゲータ越しに接続する。

> メモ: Pekko では `systemGuardian` の初期化時に `guardian` を参照するため、`system` 生成前に `user` を確定させるアプローチ（上記ステップ2→3）を採用する。

## API 設計
- `ActorSystem::actor_of` / `spawn` 等の公開APIは `/user` 配下のみ作成可能。
- 新設 `ActorSystem::system_actor_of` は crate 内（`pub(crate)`）公開に留め、`/system` の子生成専用とする。
- ActorPath には `/`, `/user`, `/system`, `/temp`, `/deadLetters` を予約値として追加し、ユーザが衝突させた場合はエラー。
- 拡張やテスト用に `register_extra_top_level(name, ref)` を用意し、`root_guardian.start()` 実行前（ActorSystem 初期化完了前）のみ成功させる。起動後の呼び出しは明示的にエラー＋警告ログを返す。

### ExtraTopLevel 登録エラー
```rust
pub enum RegisterExtraTopLevelError {
    AlreadyStarted,
    ReservedName(String),
    DuplicateName(String),
}
```
- API: `fn register_extra_top_level(&mut self, name: &str, actor: ActorRefGeneric<TB>) -> Result<(), RegisterExtraTopLevelError>`
- `AlreadyStarted`: `root_guardian.start()` 後の登録試行時に返し、警告ログも出す。
- `ReservedName`: `/user`, `/system`, `/temp`, `/deadLetters` など予約済み名前と衝突した場合に返す。
- `DuplicateName`: 既存の extra top-level と衝突した場合に返す。

## rootGuardian の親管理
- Pekko では `theOneWhoWalksTheBubblesOfSpaceTime` が rootGuardian の親として存在するが、fraktor-rs では循環参照を避けるため rootGuardian の親は `None` とする。
- `root_guardian` の `get_parent()` は常に `None` を返し、障害時は rootGuardian 自身の監督戦略内で `ActorSystem::terminate` を呼び出すことでシステム終了をトリガする。
- これにより Rust の所有権モデルに適した単純なガーディアン階層を保ちながら、Pekko と同じフェイルファスト挙動を実現する。

## 監督戦略とDeathWatch
- ルート: `OneForOneStrategy` 固定（`SupervisorStrategy::Stop`）。障害をログに出力後、`ActorSystem::terminate` をただちにトリガし、ガーディアン再生成は行わない。
- ユーザ: 設定可能（Pekko同様に `SupervisorStrategyConfigurator` 相当を受け付ける）。
- システム: デフォルト `SupervisorStrategy::default()` を用い、`StopChild` 相当のメッセージで子を停止可能とする。
- DeathWatch 連鎖:
  - `/system` が `/user` を watch し、`Terminated` を受けたら TerminationHook 配信→自己停止フェーズに移行する。
  - ルートが `/system` を watch し、`Terminated` を受けたら ActorSystem を終了済みとしてマークし、余剰リソースを解放する。

### 監督戦略カスタマイズ API
```rust
pub struct ActorSystemConfig<TB: RuntimeToolbox> {
    pub user_guardian_props: PropsGeneric<TB>,
    pub user_guardian_strategy: Option<Box<dyn SupervisorStrategy<TB>>>,
}

pub enum SupervisorStrategyConfigError {
    SystemGuardianNotCustomizable,
    RootGuardianNotCustomizable,
    InvalidStrategy(String),
}
```
- `/user` だけが `user_guardian_strategy` を通じてカスタム戦略を受け付ける。
- `/system` とルートがカスタマイズされそうになった場合は `SupervisorStrategyConfigError::*` を返し、固定値にフォールバックする。
- 戦略が無効（例: タイムアウトが負値）の場合は `InvalidStrategy` を返し、構築自体を失敗させる。

## TerminationHook プロトコル
- `SystemGuardian` 専用メッセージ
  - `RegisterTerminationHook`（sender をセットへ追加し watch 登録）
  - `TerminationHook`
  - `TerminationHookDone`
- `/system` 監視下の内部アクター（クラスタ、リモート、Schedulerなど）は起動時に Register を送信。SystemGuardian 側で sender を watch する。
- `/user` ガーディアンの `Terminated` 通知を受けたら SystemGuardian は `TerminationHook` を全フックへ送付し、`TerminationHookDone` または `Terminated` を待つ。
- タイムアウトや応答欠如があった場合は警告を出しつつ停止フローを進め、全完了後にイベントログ停止→自己停止する。

## Guardian Behavior と StopChild
- Root/User/System いずれのガーディアンも `pre_restart` を空実装にして子アクターを失わない（Pekko のコメント “guardian MUST NOT lose its children during restart” と同様）。
- `/system` と `/user` のガーディアンは `StopChild(child)` システムメッセージを処理し、対象の子のみを停止する。これは `system.stop(actor_ref)` を実装する際の内部 API として使用する。
- Root ガーディアンは監督戦略が `Stop` を返した時点で `system.terminate()` を即座に起動するため、`StopChild` は `/user` と `/system` にのみ送信される。

### pre_restart ポリシー
```rust
impl Actor for UserGuardian {
    fn pre_restart(&mut self, _ctx: &mut Context<Self>, _cause: &ActorError, _msg: Option<&dyn Any>) {
        // no-op: leave children untouched to honor guardian semantics
    }
}
```
- これによりガーディアン再起動時も子アクターの参照・監視状態が維持される。

### StopChild システムメッセージ
```rust
pub(crate) enum GuardianSystemMessage {
    StopChild(Pid),
}
```
- `/user` および `/system` ガーディアンは `GuardianSystemMessage::StopChild(pid)` を受け取ると該当子を停止し、他の子や自身は継続する。
- `ActorSystem::stop(actor_ref)` や CoordinatedShutdown が子アクターを順次停止させる際に利用する。

## SystemGuardian の状態遷移
1. **Running**: Register/StopChild を処理し、TerminationHook 待機状態。
2. **Terminating**: `/user` の `Terminated` を受信すると遷移。全フックへ `TerminationHook` を送付し、各フックの `TerminationHookDone` または `Terminated` を待つ。
3. **Stopped**: フック集合が空になったらイベントログを停止し self を停止。ルートが `Terminated(system)` を受信して ActorSystem を終了済みとする。

## 終了シーケンス
1. `ActorSystem::terminate` または CoordinatedShutdown トリガ。
2. ルートが `/user` ガーディアンへ `StopChild` 相当を送り、公開アクターを順次停止させる。
3. `/system` ガーディアンが `/user` の `Terminated` 通知を受けて TerminationHook フェーズへ遷移し、全フック完了後に自身を停止する。
4. ルートが `/system` の `Terminated` 通知を受け、イベントストリーム停止・残処理を行い ActorSystem を終了済みとしてマークする。

## テスト方針
- `guardian/tests.rs` で以下を検証:
  - 予約パス生成とエラー処理
  - ルート→ユーザ→システムの生成順序とDeathWatchリンク
  - `/user` API と `/system` 内部APIの境界
  - TerminationHook プロトコルのハッピーパスとタイムアウト
  - 終了シーケンス中のログ・イベントストリーム停止
  - `/user` または `/system` で Escalate が返った際にルートが即座に terminate フローへ入ること
- 追加で以下のケースを Phase 4 テストに含める:
  - `register_extra_top_level` の成功・予約語衝突・重複・起動後エラー
  - TerminationHook 登録アクターが早期に停止した場合の挙動
  - `ActorSystem::terminate` の並行呼び出しや終了中の新規 spawn 試行

## 実装ノート
- 現行 `ActorSystemGeneric`（例: `modules/actor-core/src/system/base.rs`）はユーザガーディアンのみ生成しているため、`root_guardian`・`system_guardian` の参照を `SystemStateGeneric` に追加する。
- `ActorSystemGeneric::new` ではルート生成→`reserve_child("user")`→`reserve_child("system")`→DeathWatch連結→`guardian.start()`→`system_actor_of` 初期化という順序を保証する。
- 既存 API で露出している `/user` ガーディアン参照は `ActorSystem` ラッパ経由（=`/user` へのデリゲータ）に限定し、テストコード含め直接参照を段階的に排除する。
- `/temp` 向け VirtualPathContainer を本変更内で実装し、`register_temp_actor` / `unregister_temp_actor` API を ActorSystem に提供する。ask/probe をまだ公開しない場合でも、内部テストや将来機能が即座に利用できる状態を保つ。
