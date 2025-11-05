# DeathWatch 移行ガイド

このガイドでは Akka/Pekko の `DeathWatch` から cellactor-rs の `ActorContext::watch` API へ移行する際の注意点をまとめます。

## 基本構文

| Akka/Pekko | cellactor-rs |
|------------|---------------|
| `context.watch(child)` | `ctx.watch(child.actor_ref())?` |
| `context.unwatch(child)` | `ctx.unwatch(child.actor_ref())?` |
| `case Terminated(ref)` | `fn on_terminated(&mut self, ctx: &mut ActorContext<'_>, pid: Pid)` |

```scala
// Akka
context.watch(child)

def receive = {
  case Terminated(ref) => restart(ref)
}
```

```rust
// cellactor-rs
ctx.watch(child.actor_ref())?;

fn on_terminated(
  &mut self,
  ctx: &mut ActorContext<'_>,
  terminated: Pid,
) -> Result<(), ActorError> {
  ctx.log(LogLevel::Info, format!("{:?} stopped", terminated));
  let _ = self.spawn_replacement(ctx);
  Ok(())
}
```

## ベストプラクティス

1. **監視登録は `pre_start` でまとめる**: 子生成直後に `watch` を呼ぶと race を避けられます。
2. **`spawn_child_watched` を活用**: 子生成と DeathWatch 登録を同時に行い、エラーハンドリングを簡略化します。
3. **`unwatch` の呼び出しタイミング**: 長期的に不要になった監視は積極的に解除し、無駄な Terminated 通知を減らします。
4. **EventStream との併用**: system-wide な可観測性は従来通り EventStream で追跡し、局所的な復旧ロジックは DeathWatch で処理します。

## FAQ

- **Q. 既に停止した PID を `watch` するとどうなりますか？**
  - A. `watch` は `Ok(())` を返し、同じアクターに即時の `on_terminated` が配送されます。
- **Q. `on_terminated` 内で `ctx.watch` を再度呼んでも安全ですか？**
  - A. はい。ActorCell が `SystemMessage::Watch` を優先処理するため、再生成した子の監視も race なく登録できます。
- **Q. DeathWatch と SupervisorStrategy の優先順位は？**
  - A. 再起動/停止処理は従来通り SupervisorStrategy が担当し、DeathWatch はアクター自身の復旧ロジックに特化します。
