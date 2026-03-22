# 戻り値破棄パターン 調査レポート

調査日: 2026-03-22
対象: `modules/` 配下の本番コード（テストコード除外）

## 重大度: 高

### `send_system_message` の `Result<(), SendError>` 握りつぶし（計13箇所）

アクターへのシステムメッセージ送信失敗を無視しており、
**監視通知・停止・再起動がサイレントに失われる**可能性がある。

#### `system_state.rs`（4箇所）

| 行 | コード | コンテキスト |
|----|--------|------------|
| 689 | `let _ = self.send_system_message(watcher, SystemMessage::Terminated(pid));` | 死活監視通知 |
| 862 | `let _ = self.send_system_message(target, SystemMessage::Stop);` | Recreate 戦略 |
| 883 | `let _ = self.send_system_message(target, SystemMessage::Resume);` | Resume 戦略 |
| 890 | `let _ = self.send_system_message(pid, SystemMessage::Stop);` | stop_actor |

#### `system_state_shared.rs`（9箇所）

| 行 | コード | コンテキスト |
|----|--------|------------|
| 639 | `let _ = self.send_system_message(watcher, SystemMessage::Terminated(pid));` | 死活監視通知 |
| 665, 670 | `let _ = self.send_system_message(pid, SystemMessage::Stop);` | handle_failure |
| 684 | `let _ = self.send_system_message(target, SystemMessage::Stop);` | Recreate 戦略 |
| 694 | `let _ = self.send_system_message(target, SystemMessage::Stop);` | Stop 戦略 |
| 699 | `let _ = self.send_system_message(target, SystemMessage::Stop);` | Escalate 戦略 |
| 705 | `let _ = self.send_system_message(target, SystemMessage::Resume);` | Resume 戦略 |
| 841, 848 | `let _ = self.send_system_message(child_pid, SystemMessage::Stop);` | failure outcome |

#### 影響

- `Terminated` 通知の欠落で DeathWatch の親がゾンビ子アクターを永遠に待つ
- `Stop`/`Resume` の欠落でスーパーバイザー戦略が無効化され、障害アクターが放置される

#### 参照実装との比較

Pekko では `tell` は fire-and-forget だが、システムメッセージ（`Terminated` 等）は
内部キューで配信が保証される設計。fraktor-rs で `Result` を捨てているのは
設計方針なのか一時的な妥協なのかを判断する必要がある。

---

## 重大度: 中

### `unregister_actor` 内の戻り値握りつぶし

**ファイル**: `system_state_shared.rs`

| 行 | コード | 捨てている型 |
|----|--------|------------|
| 202 | `let _ = guard.actor_path_registry_mut().reserve_uid(...)` | `Result`（UID 予約） |
| 205, 210 | `let _ = self.cells.with_write(\|cells\| cells.remove(pid));` | `Option<ActorCell>` |

#### 影響

- UID 予約失敗で同じパスに新アクターが割り当てられる可能性
- セル削除は「既に削除済み」の確認を放棄

### `with_runner` の `Option<R>` 握りつぶし

**ファイル**: `manual_tick_controller.rs`

| 行 | コード | 捨てている型 |
|----|--------|------------|
| 23-25 | `self.state.with_runner(\|runner, _\| runner.inject(ticks));` | `Option<()>` |
| 30-32 | `self.state.with_runner(\|runner, scheduler\| ...);` | `Option<()>` |

#### 影響

- runner 未設定時に tick 注入・スケジューラ駆動がサイレントに失敗

---

## 重大度: 低

| ファイル | 行 | コード | 型 |
|---------|-----|--------|-----|
| `showcases-std/src/support/tick_driver.rs` | 187 | `let _ = handle.join();` | `Result<(), Box<dyn Any>>` |

サポートユーティリティのため実害は限定的。

---

## 総括

| 重大度 | 箇所数 | 主な問題 |
|--------|--------|---------|
| 高 | 13 | `send_system_message` の Result 破棄 — 監視・スーパービジョン破綻 |
| 中 | 5 | UID 予約・セル削除・tick 駆動の失敗無視 |
| 低 | 1 | スレッド join 結果の無視 |
| **計** | **19** | |
