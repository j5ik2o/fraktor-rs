# SystemState は private registry の facade として残す

`SystemState` と `SystemStateShared` は既存の external / crate-visible accessor contract を維持し、実行補助、identity/path、guardian/cells、dispatch/mailbox、event/logging、remote/provider、scheduler/lifecycle の subsystem state は private registry へ分ける。registry handle を公開したり actor system behavior を変えたりせず、後続の mailbox、EventBus、CoordinatedShutdown work を局所化するためである。

**Considered Options**

- public registry handle: public surface audit は別の関心事であり、caller が内部 state ownership に依存すべきではないため不採用。
- `SystemState` 内の field 整理だけで済ませる案: mailbox、EventBus、shutdown workstream の将来の変更競合を減らせないため不採用。
- 既存 facade の内側に private leaf registry を置く案: 互換性を維持しつつ downstream spec が参照できる内部境界を作れるため採用。
