#!/usr/bin/env python3
from __future__ import annotations

import argparse
import errno
import fcntl
import os
import pty
import re
import select
import shutil
import struct
import subprocess
import sys
import tempfile
import termios
import time
import tomllib
from pathlib import Path
from typing import Any

CSI_RE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
OSC_RE = re.compile(r"\x1b\][^\x07]*(?:\x07|\x1b\\)")
ST_RE = re.compile(r"\x1b(?:P|_|\^).*?\x1b\\", re.DOTALL)

KEY_BYTES = {
    "enter": b"\r",
    "tab": b"\t",
    "up": b"\x1b[A",
    "down": b"\x1b[B",
    "left": b"\x1b[D",
    "right": b"\x1b[C",
    "esc": b"\x1b",
    "ctrl+r": b"\x12",
    "ctrl+c": b"\x03",
}


class ScenarioError(RuntimeError):
    pass


class PtySession:
    def __init__(self, cmd: list[str], cwd: Path, rows: int, cols: int) -> None:
        self.cmd = cmd
        self.cwd = cwd
        self.rows = rows
        self.cols = cols
        self.master_fd, slave_fd = pty.openpty()
        fcntl.ioctl(slave_fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))

        env = os.environ.copy()
        env.setdefault("TERM", "xterm-256color")
        env.setdefault("COLORTERM", "truecolor")

        try:
            self.proc = subprocess.Popen(
                cmd,
                cwd=cwd,
                env=env,
                stdin=slave_fd,
                stdout=slave_fd,
                stderr=slave_fd,
                close_fds=True,
            )
        finally:
            os.close(slave_fd)

        self.raw = bytearray()
        self.cursor = 0
        self._closed = False

    def read_some(self, timeout: float) -> bool:
        ready, _, _ = select.select([self.master_fd], [], [], timeout)
        if not ready:
            return False

        try:
            chunk = os.read(self.master_fd, 65536)
        except OSError as exc:
            if exc.errno == errno.EIO:
                return False
            raise

        if not chunk:
            return False

        self.raw.extend(chunk)
        return True

    def expect(self, needle: str, timeout: float) -> None:
        needle_bytes = needle.encode("utf-8")
        deadline = time.monotonic() + timeout

        while True:
            haystack = bytes(self.raw)
            idx = haystack.find(needle_bytes, self.cursor)
            if idx != -1:
                self.cursor = idx + len(needle_bytes)
                return

            if self.proc.poll() is not None:
                while self.read_some(0.05):
                    pass
                haystack = bytes(self.raw)
                idx = haystack.find(needle_bytes, self.cursor)
                if idx != -1:
                    self.cursor = idx + len(needle_bytes)
                    return
                raise ScenarioError(
                    f"Process exited while waiting for {needle!r}.\n"
                    f"Last output:\n{self.tail_text()}"
                )

            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise ScenarioError(
                    f"Timed out waiting for {needle!r}.\n"
                    f"Last output:\n{self.tail_text()}"
                )

            self.read_some(min(0.2, remaining))

    def send_token(self, token: str) -> None:
        lower = token.lower()
        if lower.startswith("text:"):
            data = token.split(":", 1)[1].encode("utf-8")
            os.write(self.master_fd, data)
            return

        if lower.startswith("sleep:"):
            time.sleep(float(token.split(":", 1)[1]))
            return

        try:
            data = KEY_BYTES[lower]
        except KeyError as exc:
            raise ScenarioError(f"Unsupported token {token!r}") from exc

        os.write(self.master_fd, data)

    def wait_for_exit(self, timeout: float) -> None:
        deadline = time.monotonic() + timeout
        while True:
            if self.proc.poll() is not None:
                while self.read_some(0.05):
                    pass
                return

            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise ScenarioError(
                    f"Timed out waiting for process exit.\nLast output:\n{self.tail_text()}"
                )

            self.read_some(min(0.2, remaining))

    def normalized_text(self) -> str:
        text = bytes(self.raw).decode("utf-8", errors="replace")
        text = OSC_RE.sub("", text)
        text = ST_RE.sub("", text)
        text = CSI_RE.sub("", text)
        text = text.replace("\r\n", "\n").replace("\r", "\n")
        return text

    def tail_text(self, lines: int = 80) -> str:
        split = self.normalized_text().splitlines()
        return "\n".join(split[-lines:])

    def write_logs(self, prefix: Path) -> None:
        prefix.parent.mkdir(parents=True, exist_ok=True)
        raw_path = prefix.with_suffix(".raw.log")
        normalized_path = prefix.with_suffix(".normalized.log")
        raw_path.write_bytes(bytes(self.raw))
        normalized_path.write_text(self.normalized_text(), encoding="utf-8")

    def close(self) -> None:
        if self._closed:
            return
        self._closed = True

        try:
            if self.proc.poll() is None:
                self.proc.kill()
                self.proc.wait(timeout=5)
        finally:
            try:
                os.close(self.master_fd)
            except OSError:
                pass


def copy_into_workspace(src: Path, dst: Path) -> None:
    if not src.exists():
        raise ScenarioError(f"Fixture path does not exist: {src}")

    if src.is_dir():
        shutil.copytree(src, dst, symlinks=True)
    else:
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)



def repo_relative_files(root: Path) -> list[str]:
    if not root.exists():
        return []

    files: list[str] = []
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        rel = path.relative_to(root)
        if any(part.startswith(".") for part in rel.parts):
            continue
        files.append(rel.as_posix())
    files.sort()
    return files


def assert_tree(label: str, root: Path, expected: list[str]) -> None:
    actual = repo_relative_files(root)
    if actual != expected:
        raise ScenarioError(
            f"{label} mismatch for {root}:\n"
            f"expected: {expected}\n"
            f"actual:   {actual}"
        )


def run_shell(command: str, cwd: Path) -> None:
    result = subprocess.run(
        ["bash", "-c", command],
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise ScenarioError(
            f"Command failed: {command!r}\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )


def load_scenario(path: Path) -> dict[str, Any]:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def resolve_binary(explicit: str | None, repo_root: Path) -> Path:
    candidates = [explicit, os.environ.get("PIFBIP_BIN")]
    candidates.extend(
        [
            str(repo_root / "target" / "debug" / "pifbip"),
            str(repo_root / "target" / "release" / "pifbip"),
        ]
    )

    for candidate in candidates:
        if not candidate:
            continue
        path = Path(candidate)
        if path.exists():
            return path.resolve()

    raise ScenarioError("Could not find pifbip binary. Pass --binary or set PIFBIP_BIN.")


def scenario_log_prefix(repo_root: Path, scenario_path: Path) -> Path:
    name = scenario_path.relative_to(repo_root).as_posix().replace("/", "__")
    return repo_root / "target" / "cli-scenarios" / name


def run_scenario(scenario_path: Path, binary: Path, repo_root: Path) -> None:
    data = load_scenario(scenario_path)
    name = data.get("name", scenario_path.stem)
    setup = data.get("setup")
    teardown = data.get("teardown")
    workspace_copy = list(data.get("workspace_copy", []))
    binary_args = list(data["binary_args"])
    prompt = data["prompt"]
    startup = data.get("startup")
    summary = data["summary"]
    timeout = float(data.get("timeout_seconds", 20))
    rows = int(data.get("pty_rows", 40))
    cols = int(data.get("pty_cols", 120))
    steps = list(data["steps"])
    initial_source_files = list(data.get("initial_source_files", []))
    initial_destination_files = list(data.get("initial_destination_files", []))
    final_source_files = list(data.get("final_source_files", []))
    final_destination_files = list(data.get("final_destination_files", []))

    session: PtySession | None = None
    log_prefix = scenario_log_prefix(repo_root, scenario_path)
    workspace: tempfile.TemporaryDirectory[str] | None = None
    working_root = repo_root

    if workspace_copy:
        workspace = tempfile.TemporaryDirectory(prefix=f"pifbip-scenario-{name}-")
        working_root = Path(workspace.name)
        for relative in workspace_copy:
            src = repo_root / relative
            dst = working_root / relative
            copy_into_workspace(src, dst)

    source_root = (working_root / data["source_root"]).resolve()
    destination_root = (working_root / data["destination_root"]).resolve()

    print(f"==> {name} ({scenario_path.relative_to(repo_root)})")
    error: Exception | None = None
    try:
        if setup:
            run_shell(setup, working_root)

        assert_tree("Initial source tree", source_root, initial_source_files)
        assert_tree("Initial destination tree", destination_root, initial_destination_files)

        cmd = [str(binary), *binary_args]
        session = PtySession(cmd, working_root, rows=rows, cols=cols)

        if startup:
            session.expect(startup, timeout)

        total = len(steps)
        for index, step in enumerate(steps, start=1):
            filename = step["file"]
            header = f"[{index}/{total}] {filename}"
            session.expect(header, timeout)
            session.expect(prompt, timeout)
            for token in step["keys"]:
                session.send_token(token)

        session.expect(summary, timeout)
        session.wait_for_exit(timeout)
        session.write_logs(log_prefix)

        assert_tree("Final source tree", source_root, final_source_files)
        assert_tree("Final destination tree", destination_root, final_destination_files)
        print("    ok")
    except Exception as exc:
        error = exc
        raise
    finally:
        teardown_error: Exception | None = None
        if session is not None:
            session.write_logs(log_prefix)
            session.close()
        if teardown:
            try:
                run_shell(teardown, working_root)
            except Exception as exc:
                teardown_error = exc
        if workspace is not None:
            workspace.cleanup()

        if error is None and teardown_error is not None:
            raise teardown_error


def discover_scenarios(repo_root: Path, requested: list[str]) -> list[Path]:
    if requested:
        return [Path(item).resolve() for item in requested]

    return sorted((repo_root / "tests" / "scenarios").glob("*.toml"))


def main() -> int:
    parser = argparse.ArgumentParser(description="Replay interactive CLI scenarios in a PTY")
    parser.add_argument("scenarios", nargs="*", help="Scenario TOML files to run")
    parser.add_argument("--binary", help="Path to the pifbip binary to run")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent
    binary = resolve_binary(args.binary, repo_root)
    scenarios = discover_scenarios(repo_root, args.scenarios)

    if not scenarios:
        raise ScenarioError("No scenario files found")

    failures = 0
    for scenario in scenarios:
        try:
            run_scenario(scenario, binary, repo_root)
        except ScenarioError as exc:
            failures += 1
            print(f"    FAILED: {exc}", file=sys.stderr)

    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
