#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_span;

use std::path::{Path, PathBuf};

use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{FileName, RealFileName, source_map::SourceMap};

dylint_linting::impl_late_lint! {
  pub ACTOR_LOCK_CONSTRUCTION,
  Warn,
  "forbid direct fixed-family lock construction in actor production code",
  ActorLockConstruction
}

pub struct ActorLockConstruction;

const PROHIBITED_PATTERNS: &[(&str, &str)] = &[
  ("SpinSyncMutex::new(", "direct `SpinSyncMutex::new(...)`"),
  ("SpinSyncRwLock::new(", "direct `SpinSyncRwLock::new(...)`"),
  ("new_with_driver::<SpinSyncMutex", "fixed `SpinSyncMutex` driver selection"),
  ("new_with_driver::<SpinSyncRwLock", "fixed `SpinSyncRwLock` driver selection"),
  ("new_with_builtin_lock(", "fixed-family helper alias `new_with_builtin_lock(...)`"),
];

const ALLOWLIST_PATHS: &[&str] = &[
  "modules/actor-core/src/core/kernel/system/lock_provider/",
  "modules/actor-adaptor-std/src/std/system/lock_provider/",
  "modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher_shared.rs",
  "modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared.rs",
  "modules/actor-core/src/core/kernel/dispatch/dispatcher/shared_message_queue.rs",
  "modules/actor-core/src/core/kernel/dispatch/mailbox/mailbox_queue_state.rs",
  "modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_priority_message_queue.rs",
  "modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_stable_priority_message_queue.rs",
  "modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_control_aware_message_queue.rs",
  "modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_deque_message_queue.rs",
  "modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_priority_message_queue.rs",
  "modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_stable_priority_message_queue.rs",
  "modules/actor-core/src/core/kernel/event/stream/event_stream_shared.rs",
  "modules/actor-core/src/core/kernel/event/stream/event_stream_subscriber.rs",
  "modules/actor-core/src/core/kernel/actor/actor_ref/actor_ref_sender_shared.rs",
  "modules/actor-core/src/core/kernel/actor/actor_ref/dead_letter/dead_letter_shared.rs",
  "modules/actor-core/src/core/kernel/actor/actor_shared.rs",
  "modules/actor-core/src/core/kernel/actor/props/factory_shared.rs",
  "modules/actor-core/src/core/kernel/actor/context_pipe/waker.rs",
  "modules/actor-core/src/core/kernel/actor/messaging/message_invoker/invoker_shared.rs",
  "modules/actor-core/src/core/kernel/actor/messaging/message_invoker/middleware_shared.rs",
  "modules/actor-core/src/core/kernel/util/futures/actor_future_shared.rs",
  "modules/actor-core/src/core/kernel/pattern/circuit_breaker_shared.rs",
  "modules/actor-core/src/core/kernel/system/state/system_state_shared.rs",
  "modules/actor-core/src/core/kernel/system/termination_state.rs",
  "modules/actor-core/src/core/kernel/system/coordinated_shutdown.rs",
  "modules/actor-core/src/core/kernel/system/remote/remote_watch_hook_shared.rs",
  "modules/actor-core/src/core/kernel/system/remote/remote_watch_hook_dyn_shared.rs",
  "modules/actor-core/src/core/kernel/system/cells_shared.rs",
  "modules/actor-core/src/core/kernel/serialization/serialization_registry/registry.rs",
  "modules/actor-core/src/core/kernel/serialization/extension_shared.rs",
  "modules/actor-core/src/core/kernel/actor/actor_ref_provider/actor_ref_provider_shared.rs",
  "modules/actor-core/src/core/kernel/actor/scheduler/diagnostics/diagnostics_registry.rs",
  "modules/actor-core/src/core/typed/behavior.rs",
  "modules/actor-core/src/core/typed/receptionist.rs",
  "modules/actor-core/src/core/typed/pubsub/topic.rs",
  "modules/actor-core/src/core/typed/message_adapter/adapter_envelope.rs",
  "modules/actor-core/src/core/typed/message_adapter/adapt_message.rs",
  "modules/actor-core/src/core/typed/delivery/consumer_controller.rs",
  "modules/actor-core/src/core/typed/delivery/producer_controller.rs",
  "modules/actor-core/src/core/typed/delivery/work_pulling_producer_controller.rs",
  "modules/actor-core/src/core/typed/dsl/fsm_builder.rs",
  "modules/actor-core/src/core/typed/dsl/behaviors.rs",
  "modules/actor-core/src/core/typed/dsl/routing/group_router.rs",
  "modules/actor-core/src/core/typed/dsl/routing/pool_router.rs",
  "modules/actor-core/src/core/typed/dsl/routing/tail_chopping_router_builder.rs",
  "modules/actor-core/src/core/typed/dsl/routing/scatter_gather_first_completed_router_builder.rs",
  "modules/actor-core/src/core/typed/dsl/routing/balancing_pool_router_builder.rs",
  "modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/",
  "modules/actor-adaptor-std/src/std/tick_driver.rs",
];

impl<'tcx> LateLintPass<'tcx> for ActorLockConstruction {
  fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &Expr<'tcx>) {
    if expr.span.from_expansion() || !matches!(expr.kind, ExprKind::Call(..)) {
      return;
    }

    let sm = cx.tcx.sess.source_map();
    let Some(path) = file_path_from_span(sm, expr.span) else {
      return;
    };
    let normalized = normalize_path(&path);

    if !is_candidate_path(&normalized) || is_ignored_test_path(&normalized) || is_allowlisted_path(&normalized) {
      return;
    }

    let Ok(snippet) = sm.span_to_snippet(expr.span) else {
      return;
    };
    let matches = matched_patterns(&snippet);
    if matches.is_empty() {
      return;
    }

    cx.span_lint(ACTOR_LOCK_CONSTRUCTION, expr.span, |diag| {
      diag.primary_message("actor-* の production code で固定 lock family の直構築は禁止です");
      diag.note(format!("対象ファイル: {}", normalized));
      diag.note(format!("検出内容: {}", matches.join(", ")));
      diag.help(
        "修正手順: 1. `ActorLockProvider` か許可済み shared wrapper の constructor boundary へ寄せる 2. fixed-family helper alias を通常 caller から消す 3. 低レベル例外が本当に必要なら allow-list 候補として migration メモへ追加する",
      );
      diag.note(
        "スコープ: 違反箇所と、その生成境界を受け渡す最小限の constructor だけを変更し、他の runtime path は変更しないこと",
      );
    });
  }
}

fn matched_patterns(snippet: &str) -> Vec<&'static str> {
  PROHIBITED_PATTERNS
    .iter()
    .filter_map(|(needle, label)| snippet.contains(needle).then_some(*label))
    .collect()
}

fn normalize_path(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

fn is_candidate_path(path: &str) -> bool {
  path.contains("/modules/actor-core/src/")
    || path.contains("/modules/actor-adaptor-std/src/")
    || path.contains("tests/ui/")
}

fn is_ignored_test_path(path: &str) -> bool {
  if path.contains("tests/ui/") {
    return false;
  }

  path.contains("/tests/")
    || path.contains("/benches/")
    || path.ends_with("/tests.rs")
    || path.ends_with("_tests.rs")
    || path.contains("/target/")
}

fn is_allowlisted_path(path: &str) -> bool {
  ALLOWLIST_PATHS.iter().any(|allowed| path.contains(allowed))
}

fn file_path_from_span(sm: &SourceMap, span: rustc_span::Span) -> Option<PathBuf> {
  match sm.span_to_filename(span) {
    FileName::Real(RealFileName::LocalPath(path)) => Some(path.to_path_buf()),
    _ => None,
  }
}
