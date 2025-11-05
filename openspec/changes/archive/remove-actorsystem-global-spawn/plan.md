# remove-actorsystem-global-spawn 実装計画

## 目的
ActorSystemの公開APIからアクター操作メソッド（`spawn`, `spawn_child`, `actor_ref`, `children`, `stop_actor`）を削除し、ActorContext経由でのみアクター操作を可能にする。

## フェーズ分割

### フェーズ1: API 削除
1. **API 可視性変更**
   - `ActorSystemGeneric::spawn`, `spawn_child`, `actor_ref`, `children`, `stop_actor` を `pub(crate)` に変更
   - `modules/actor-std/src/system/base.rs` から対応するメソッドを削除
   - コンパイルエラー発生箇所をリストアップ

2. **既存コードの書き換え**
   - テストコードを ActorContext 経由のパターンに書き換え
   - サンプルコードを ActorContext 経由のパターンに書き換え
   - 内部実装で必要な箇所は `pub(crate)` API を使用（クレート内なので問題なし）

### フェーズ2: ドキュメント整備
1. **CHANGELOG 更新**
   - BREAKING CHANGE として記載
   - 移行ガイドを追加

2. **ドキュメント/サンプル更新**
   - README のサンプルコードを更新
   - examples を ActorContext パターンに更新
   - API ドキュメントを更新

### フェーズ3: 検証
1. **CI/CD**
   - `makers ci-check` が全てパス
   - 全テストスイートが成功
   - カバレッジが維持されている

2. **OpenSpec 検証**
   - `openspec validate remove-actorsystem-global-spawn --strict` を実行

## 決定事項
- シンプルに API 削除のみを行う（SpawnProtocol/SpawnClientは別の変更提案として分離）
- 既存テストは ActorContext 経由のパターンに書き換える
- テスト用途での PID ベース API は `#[cfg(test)]` での限定公開を検討
- ユーザーが自由にガーディアンパターンを実装できるよう、制約を最小限にする

## 推奨パターン

### パターン1: 起動時に全て生成
```rust
struct MyGuardian {
    worker_ref: Option<ActorRef<TB>>,
}

impl Actor for MyGuardian {
    fn pre_start(&mut self, ctx: &mut ActorContext) {
        let child = ctx.spawn_child(&worker_props());
        self.worker_ref = Some(child.actor_ref());
    }
}
```

### パターン2: メッセージ駆動の動的生成（ユーザーが実装）
```rust
enum GuardianCommand {
    SpawnWorker { props: Props<TB>, reply_to: ActorRef<TB> },
}

impl Actor for MyGuardian {
    fn receive(&mut self, ctx: &mut ActorContext, msg: AnyMessage) {
        if let Some(cmd) = msg.downcast_ref::<GuardianCommand>() {
            match cmd {
                GuardianCommand::SpawnWorker { props, reply_to } => {
                    match ctx.spawn_child(props) {
                        Ok(child) => reply_to.tell(child.actor_ref()),
                        Err(e) => reply_to.tell(e),
                    }
                }
            }
        }
    }
}
```

## リスクと対応
- **既存ユーザーへの影響**: BREAKING CHANGE として明確に周知、移行ガイドを充実
- **テスト大量更新**: ガーディアンパターンのテンプレート提供で工数削減
- **利用者ドキュメント差分**: examples/README を同時更新し、周知を徹底
