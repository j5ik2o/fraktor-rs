## REMOVED Requirements

### Requirement: RcShared
**Reason**: workspace 内で未使用
**Migration**: なし（使用箇所なし）

### Requirement: StaticRefShared
**Reason**: workspace 内で未使用
**Migration**: なし

### Requirement: SharedFactory / SharedFn
**Reason**: workspace 内で未使用
**Migration**: なし

### Requirement: AtomicFlag
**Reason**: workspace 内で未使用
**Migration**: なし

### Requirement: AtomicState
**Reason**: workspace 内で未使用
**Migration**: なし

### Requirement: InterruptPolicy / CriticalSectionInterruptPolicy / NeverInterruptPolicy
**Reason**: workspace 内で未使用
**Migration**: なし

### Requirement: AsyncMutexLike / SpinAsyncMutex
**Reason**: workspace 内で未使用（clippy.toml のコメントのみ）
**Migration**: clippy.toml のコメントから AsyncMutexLike の参照を除去

### Requirement: std::collections (MpscBackend)
**Reason**: workspace 内で未使用
**Migration**: なし
