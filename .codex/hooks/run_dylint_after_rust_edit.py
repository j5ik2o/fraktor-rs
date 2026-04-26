#!/usr/bin/env python3

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

PATCH_FILE_PATTERN = re.compile(r"^\*\*\* (?:Update|Add|Delete) File: (.+)$")
DIRECT_RUST_PATH_KEYS = ("file_path", "path", "target_file")
HOOK_LOCK_PATH = Path(".takt/.codex-hook-dylint.lock")
CI_LOCK_PATH = Path(".takt/.ci-check.lock")
LOCK_WAIT_TIMEOUT_SEC = 1800
LOCK_POLL_INTERVAL_SEC = 1.0
CI_COMMAND = ("./scripts/ci-check.sh", "ai", "dylint")
MAX_FAILURE_LINES = 160
MAX_FAILURE_CHARS = 12000


def main() -> int:
    payload = load_payload()
    if payload is None:
        return block("Codex hook の入力 JSON を解釈できませんでした。")

    tool_input = payload.get("tool_input")
    if not isinstance(tool_input, dict):
        return 0

    rust_paths = extract_rust_paths(tool_input)
    if not rust_paths:
        return 0

    repo_root = resolve_repo_root(payload)
    if repo_root is None:
        return block("Git ルートを特定できなかったため、自動 dylint を実行できませんでした。")

    try:
        run_auto_dylint(repo_root)
    except HookFailure as failure:
        touched_paths = ", ".join(rust_paths)
        return block(
            "Rust ファイル編集後の自動 `./scripts/ci-check.sh ai dylint` が失敗しました。\n"
            f"対象: {touched_paths}\n\n"
            f"{failure.message}"
        )

    return 0


def load_payload() -> dict[str, object] | None:
    try:
        payload = json.load(sys.stdin)
    except json.JSONDecodeError:
        return None
    if isinstance(payload, dict):
        return payload
    return None


def find_rust_paths(command: str) -> list[str]:
    rust_paths: list[str] = []
    for line in command.splitlines():
        match = PATCH_FILE_PATTERN.match(line)
        if match is None:
            continue
        path = match.group(1).strip()
        if path.endswith(".rs"):
            rust_paths.append(path)
    return rust_paths


def extract_rust_paths(tool_input: dict[str, object]) -> list[str]:
    rust_paths: list[str] = []

    for key in DIRECT_RUST_PATH_KEYS:
        value = tool_input.get(key)
        if isinstance(value, str) and value.endswith(".rs"):
            rust_paths.append(value)

    patch_text = extract_patch_text(tool_input)
    if patch_text is not None:
        rust_paths.extend(find_rust_paths(patch_text))

    return deduplicate_paths(rust_paths)


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


def run_auto_dylint(repo_root: Path) -> None:
    repo_hook_lock_path = repo_root / HOOK_LOCK_PATH
    repo_ci_lock_path = repo_root / CI_LOCK_PATH
    repo_hook_lock_path.parent.mkdir(parents=True, exist_ok=True)

    with FileLock(repo_hook_lock_path, "codex hook dylint lock"):
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
        pid = read_lock_pid(lock_path)
        if pid is None:
            return
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
            try:
                self.fd = os.open(
                    self.lock_path,
                    os.O_WRONLY | os.O_CREAT | os.O_EXCL,
                    0o600,
                )
                os.write(self.fd, f"{os.getpid()}\n".encode("utf-8"))
                return self
            except FileExistsError:
                pid = read_lock_pid(self.lock_path)
                if pid is None or not process_exists(pid):
                    remove_stale_lock(self.lock_path)
                    continue
                if time.monotonic() >= deadline:
                    raise HookFailure(f"{self.label} の待機がタイムアウトしました。")
                time.sleep(LOCK_POLL_INTERVAL_SEC)

    def __exit__(self, exc_type, exc, traceback) -> None:
        if self.fd is not None:
            os.close(self.fd)
        try:
            self.lock_path.unlink()
        except FileNotFoundError:
            pass


def read_lock_pid(lock_path: Path) -> int | None:
    if not lock_path.exists():
        return None

    try:
        pid_text = lock_path.read_text(encoding="utf-8").strip()
    except OSError:
        return None

    if not pid_text:
        return None

    try:
        return int(pid_text)
    except ValueError:
        return None


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


def block(message: str) -> int:
    print(json.dumps({
        "should_block": True,
        "reason": message,
    }, ensure_ascii=False))
    return 2


class HookFailure(Exception):
    def __init__(self, message: str) -> None:
        super().__init__(message)
        self.message = message


if __name__ == "__main__":
    raise SystemExit(main())
