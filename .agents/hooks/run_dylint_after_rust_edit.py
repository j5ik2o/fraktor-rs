#!/usr/bin/env python3
"""AI エージェント PostToolUse hook: Rust ファイル編集後に dylint を自動実行する。

Claude Code と Codex CLI で共通利用するため、エージェント種別を `--agent`
引数で切り替える。エージェントごとに異なるのは以下の3点のみ:

* tool_input から編集対象パスを抽出する方法 (Claude は `file_path` のみ、
  Codex は `apply_patch` コマンド本文も解釈する)
* 自身の hook 用ロックファイルの配置先
* 失敗時にエージェントへ返すブロック応答の形式 (Claude は stderr へ書き出し
  て exit 2、Codex は `{should_block, reason}` の JSON を stdout へ出力)

排他制御: ci-check.sh は `target/.ci-check.lock` で多重起動を弾くため、
本 hook も先行する ci-check.sh の終了を待ってから dylint を起動する。
"""

from __future__ import annotations

import argparse
import fcntl
import json
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

CI_LOCK_PATH = Path("target/.ci-check.lock")
LOCK_WAIT_TIMEOUT_SEC = 1800
LOCK_POLL_INTERVAL_SEC = 1.0
CI_COMMAND = ("./scripts/ci-check.sh", "ai", "dylint")
MAX_FAILURE_LINES = 160
MAX_FAILURE_CHARS = 12000

PATCH_FILE_PATTERN = re.compile(r"^\*\*\* (?:Update|Add|Delete) File: (.+)$")
DIRECT_RUST_PATH_KEYS = ("file_path", "path", "target_file")


@dataclass(frozen=True)
class AgentProfile:
    name: str
    label: str
    hook_lock_path: Path
    extract_rust_paths: Callable[[dict[str, object]], list[str]]
    block: Callable[[str], int]


def main() -> int:
    args = parse_args()
    profile = AGENT_PROFILES[args.agent]

    payload = load_payload()
    if payload is None:
        return profile.block(f"{profile.label} hook の入力 JSON を解釈できませんでした。")

    tool_input = payload.get("tool_input")
    if not isinstance(tool_input, dict):
        return 0

    rust_paths = profile.extract_rust_paths(tool_input)
    if not rust_paths:
        return 0

    repo_root = resolve_repo_root(payload)
    if repo_root is None:
        return profile.block("Git ルートを特定できなかったため、自動 dylint を実行できませんでした。")

    try:
        run_auto_dylint(repo_root, profile.hook_lock_path, profile.label)
    except HookFailure as failure:
        touched_paths = ", ".join(rust_paths)
        return profile.block(
            "Rust ファイル編集後の自動 `./scripts/ci-check.sh ai dylint` が失敗しました。\n"
            f"対象: {touched_paths}\n\n"
            f"{failure.message}"
        )

    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="dylint after Rust edit hook")
    parser.add_argument(
        "--agent",
        required=True,
        choices=sorted(AGENT_PROFILES.keys()),
        help="呼び出し元のエージェント種別",
    )
    return parser.parse_args()


def load_payload() -> dict[str, object] | None:
    try:
        payload = json.load(sys.stdin)
    except json.JSONDecodeError:
        return None
    if isinstance(payload, dict):
        return payload
    return None


def extract_rust_paths_claude(tool_input: dict[str, object]) -> list[str]:
    """Claude Code (Edit / Write / MultiEdit) の tool_input を解釈する。"""
    rust_paths: list[str] = []
    file_path = tool_input.get("file_path")
    if isinstance(file_path, str) and file_path.endswith(".rs"):
        rust_paths.append(file_path)
    return rust_paths


def extract_rust_paths_codex(tool_input: dict[str, object]) -> list[str]:
    """Codex CLI の tool_input (apply_patch / Edit / Write) を解釈する。"""
    rust_paths: list[str] = []

    for key in DIRECT_RUST_PATH_KEYS:
        value = tool_input.get(key)
        if isinstance(value, str) and value.endswith(".rs"):
            rust_paths.append(value)

    patch_text = extract_patch_text(tool_input)
    if patch_text is not None:
        rust_paths.extend(find_rust_paths_in_patch(patch_text))

    return deduplicate_paths(rust_paths)


def find_rust_paths_in_patch(command: str) -> list[str]:
    rust_paths: list[str] = []
    for line in command.splitlines():
        match = PATCH_FILE_PATTERN.match(line)
        if match is None:
            continue
        path = match.group(1).strip()
        if path.endswith(".rs"):
            rust_paths.append(path)
    return rust_paths


def extract_patch_text(tool_input: dict[str, object]) -> str | None:
    command = tool_input.get("command")
    if isinstance(command, str):
        return command
    if isinstance(command, list):
        command_parts = [part for part in command if isinstance(part, str)]
        if len(command_parts) >= 2 and command_parts[0] == "apply_patch":
            return command_parts[1]
        if len(command_parts) == 1:
            return command_parts[0]

    patch_input = tool_input.get("input")
    if isinstance(patch_input, str):
        return patch_input

    return None


def deduplicate_paths(paths: list[str]) -> list[str]:
    unique_paths: list[str] = []
    seen_paths: set[str] = set()
    for path in paths:
        if path in seen_paths:
            continue
        seen_paths.add(path)
        unique_paths.append(path)
    return unique_paths


def resolve_repo_root(payload: dict[str, object]) -> Path | None:
    cwd = payload.get("cwd")
    if not isinstance(cwd, str) or not cwd:
        cwd = "."

    completed = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        cwd=cwd,
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        return None

    repo_root = completed.stdout.strip()
    if not repo_root:
        return None
    return Path(repo_root)


def run_auto_dylint(repo_root: Path, hook_lock_relative: Path, lock_label: str) -> None:
    repo_hook_lock_path = repo_root / hook_lock_relative
    repo_ci_lock_path = repo_root / CI_LOCK_PATH
    repo_hook_lock_path.parent.mkdir(parents=True, exist_ok=True)

    with FileLock(repo_hook_lock_path, lock_label):
        wait_for_lock_release(repo_ci_lock_path, "ci-check.sh")
        completed = subprocess.run(
            CI_COMMAND,
            cwd=repo_root,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
            env=build_ci_environment(),
        )

    if completed.returncode == 0:
        return

    raise HookFailure(summarize_failure_output(completed))


def build_ci_environment() -> dict[str, str]:
    environment = dict(os.environ)
    environment.setdefault("CI_CHECK_HEARTBEAT", "0")
    return environment


def wait_for_lock_release(lock_path: Path, label: str) -> None:
    deadline = time.monotonic() + LOCK_WAIT_TIMEOUT_SEC
    while True:
        exists, pid = read_lock_pid(lock_path)
        if not exists:
            return
        if pid is None:
            if time.monotonic() >= deadline:
                raise HookFailure(f"{label} のロック情報が解決できないままタイムアウトしました。")
            time.sleep(LOCK_POLL_INTERVAL_SEC)
            continue
        if not process_exists(pid):
            remove_stale_lock(lock_path)
            return
        if time.monotonic() >= deadline:
            raise HookFailure(f"{label} のロック待機がタイムアウトしました。")
        time.sleep(LOCK_POLL_INTERVAL_SEC)


class FileLock:
    def __init__(self, lock_path: Path, label: str) -> None:
        self.lock_path = lock_path
        self.label = label
        self.fd: int | None = None

    def __enter__(self) -> "FileLock":
        deadline = time.monotonic() + LOCK_WAIT_TIMEOUT_SEC
        while True:
            fd = os.open(
                self.lock_path,
                os.O_RDWR | os.O_CREAT,
                0o600,
            )
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
                write_lock_pid(fd)
                self.fd = fd
                return self
            except BlockingIOError:
                os.close(fd)
                if time.monotonic() >= deadline:
                    raise HookFailure(f"{self.label} の待機がタイムアウトしました。")
                time.sleep(LOCK_POLL_INTERVAL_SEC)
            except OSError:
                os.close(fd)
                raise

    def __exit__(self, exc_type, exc, traceback) -> None:
        if self.fd is not None:
            try:
                fcntl.flock(self.fd, fcntl.LOCK_UN)
            finally:
                os.close(self.fd)


def write_lock_pid(fd: int) -> None:
    os.ftruncate(fd, 0)
    os.lseek(fd, 0, os.SEEK_SET)
    os.write(fd, f"{os.getpid()}\n".encode("utf-8"))
    os.fsync(fd)


def read_lock_pid(lock_path: Path) -> tuple[bool, int | None]:
    if not lock_path.exists():
        return False, None

    try:
        pid_text = lock_path.read_text(encoding="utf-8").strip()
    except OSError:
        return True, None

    if not pid_text:
        return True, None

    try:
        return True, int(pid_text)
    except ValueError:
        return True, None


def process_exists(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def remove_stale_lock(lock_path: Path) -> None:
    try:
        lock_path.unlink()
    except FileNotFoundError:
        pass


def summarize_failure_output(completed: subprocess.CompletedProcess[str]) -> str:
    lines: list[str] = []
    for text in (completed.stdout, completed.stderr):
        if not text:
            continue
        lines.extend(line.rstrip() for line in text.splitlines())

    if not lines:
        return f"`{' '.join(CI_COMMAND)}` が終了コード {completed.returncode} で失敗しました。"

    tail = lines[-MAX_FAILURE_LINES:]
    message = "\n".join(tail).strip()
    if len(message) > MAX_FAILURE_CHARS:
        message = message[-MAX_FAILURE_CHARS:]
        first_newline = message.find("\n")
        if first_newline != -1:
            message = message[first_newline + 1 :]
    return message


def block_claude(message: str) -> int:
    print(message, file=sys.stderr)
    return 2


def block_codex(message: str) -> int:
    print(json.dumps({
        "should_block": True,
        "reason": message,
    }, ensure_ascii=False))
    return 2


class HookFailure(Exception):
    def __init__(self, message: str) -> None:
        super().__init__(message)
        self.message = message


AGENT_PROFILES: dict[str, AgentProfile] = {
    "claude": AgentProfile(
        name="claude",
        label="Claude",
        hook_lock_path=Path(".claude/dylint-hook.lock"),
        extract_rust_paths=extract_rust_paths_claude,
        block=block_claude,
    ),
    "codex": AgentProfile(
        name="codex",
        label="Codex",
        hook_lock_path=Path(".codex/dylint-hook.lock"),
        extract_rust_paths=extract_rust_paths_codex,
        block=block_codex,
    ),
}


if __name__ == "__main__":
    raise SystemExit(main())
