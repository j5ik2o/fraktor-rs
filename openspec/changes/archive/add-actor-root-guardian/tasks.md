## 実装タスク（優先順位順）

### Phase 1: 基盤構造（必須）
- [ ] ActorPath/ActorRef を `/`, `/user`, `/system`, `/deadLetters`, `/temp` の予約パスに対応させ、ActorSystem 起動前にだけ追加トップレベルを登録できるようにする
- [ ] `SystemStateGeneric` などランタイム内部構造に `root_guardian`・`system_guardian` の参照を追加し、既存 `user_guardian` との整合を確保する
- [ ] `register_extra_top_level` API を実装し、`AlreadyStarted` / `ReservedName` / `DuplicateName` エラーを返すようにする
- [ ] `/temp` 用 VirtualPathContainer と `register_temp_actor` / `unregister_temp_actor` API を実装する

### Phase 2: ガーディアン初期化と監督（必須）
- [ ] ActorSystem 初期化フローを「root → user → system」の順で明示的に生成し、system 作成時に user 参照を渡す
- [ ] DeathWatch 連結を実装（`system` が `user` を watch、`root` が `system` を watch）し、Typed API (`ActorSystem::tell` など) を `/user` デリゲータ経由に統一する
- [ ] 監督戦略を層別に実装（root=Stop固定、system=default固定、user=設定可能）し、無効なユーザ戦略を検証してエラーを返す
- [ ] Guardian の `pre_restart` を空実装とし、`StopChild` システムメッセージで任意の子を停止できる内部 API を実装する

### Phase 3: API境界と終了処理（必須）
- [ ] 公開API（`actor_of`/`spawn` 等）を `/user` 配下のみに制限し、`pub(crate)` な `system_actor_of` で `/system` 子のみ生成できるようにする
- [ ] SystemGuardian の状態機械（Running → Terminating → Stopped）と `RegisterTerminationHook`/`TerminationHook`/`TerminationHookDone` のハンドリングを実装し、フックタイムアウト時に強制解除する
- [ ] CoordinatedShutdown を考慮した終了シーケンス `/user` → `/system` → ルートを実装し、並行 `ActorSystem::terminate` 呼び出し時にも一貫した結果になるようガードする

### Phase 4: テストと検証（必須）
- [ ] `guardian/tests.rs` に予約パス・追加トップレベル登録（成功/エラー）・生成順序・DeathWatch 連結・API 境界・`StopChild` ハンドリングのテストを追加する
- [ ] TerminationHook のハッピーパス／タイムアウト／事前停止シナリオ、および `/user`/`/system` の Escalate 発生時にルートが即 terminate へ遷移するテスト、guardian 再起動時に子を維持するテストを追加する
- [ ] `ActorSystem::terminate` 並行呼び出し、終了中の新規 spawn 拒否、既存 system_lifecycle 系テストの更新を実施する
