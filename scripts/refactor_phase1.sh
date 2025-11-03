#!/bin/bash
set -e

cd "$(dirname "$0")/../modules/actor-core/src"

echo "=== Phase 1-1: Creating root files and directories ==="

# Create root files with module documentation
cat > messaging.rs <<'EOF'
//! Messaging package.
//!
//! This module contains message handling, type erasure, and Ask/Tell patterns.

EOF

cat > mailbox.rs <<'EOF'
//! Mailbox package.
//!
//! This module contains message queue implementations and configurations.

EOF

cat > supervision.rs <<'EOF'
//! Supervision package.
//!
//! This module contains error handling and restart strategies.

EOF

cat > props.rs <<'EOF'
//! Props package.
//!
//! This module contains actor spawning configuration.

EOF

cat > spawn.rs <<'EOF'
//! Spawn package.
//!
//! This module contains actor spawning execution and errors.

EOF

cat > system.rs <<'EOF'
//! System package.
//!
//! This module contains actor system management.

EOF

cat > eventstream.rs <<'EOF'
//! Event stream package.
//!
//! This module contains event publishing and subscription.

EOF

cat > lifecycle.rs <<'EOF'
//! Lifecycle package.
//!
//! This module contains actor lifecycle events and stages.

EOF

cat > deadletter.rs <<'EOF'
//! Dead letter package.
//!
//! This module contains undeliverable message handling.

EOF

cat > logging.rs <<'EOF'
//! Logging package.
//!
//! This module contains log events and subscribers.

EOF

cat > futures.rs <<'EOF'
//! Futures package.
//!
//! This module contains Future integration.

EOF

cat > error.rs <<'EOF'
//! Error package.
//!
//! This module contains error types.

EOF

echo "Root files created successfully"
