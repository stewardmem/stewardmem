#!/usr/bin/env python3
"""sourcekit-lsp wrapper: root the LSP at the Swift package, not the repo.

WHY THIS EXISTS (measured 2026-11-20, every claim below reproduced with a
standalone LSP probe harness):

  Zed starts ONE sourcekit-lsp per workspace, rooted at the workspace root
  (brain-server — a Rust repo with no Package.swift). sourcekit-lsp does not
  create SwiftPM workspaces for packages NESTED under a non-package root, so
  every Swift file that imports a SwiftPM module gets fallback compiler
  arguments: `No such module 'MLXLMCommon'`, `No such module 'Testing'`, …
  The code is fine — both packages build and test green from the toolchain.

  sourcekit-lsp DOES honor two mechanisms this wrapper uses:

   1. The ROOT workspace gets full preparation (including test-target build
      descriptions). Proven: rooted at shell-macos-mlx, EngineFloorPins and
      ThinkingBudgetPins (which `import Testing`) diagnose CLEAN.
   2. `workspace/didChangeWorkspaceFolders` creates real SwiftPM workspaces
      for dynamically ADDED folders — initialize-time extra folders are
      ignored, which is why this is a notification, not an initialize edit.

  The asymmetric wrinkle (also measured): an ADDED workspace serves source
  files and the shell-macos TEST target fine, but NOT the mlx test target
  (`import Testing` fails there — its test target's dependencies are all
  remote-package products). So the ROOT is shell-macos-mlx (the package that
  needs root-class preparation) and shell-macos is ADDED.

  Residual, pre-existing, NOT caused by this wrapper: brain-afm/main.swift
  and BridgeProtocolTests.swift show ~4 phantom type errors around the
  @Generable macro surface. They appear identically with the LSP rooted AT
  shell-macos itself (i.e. in the "open the package as its own workspace"
  ideal), so they are a sourcekit-lsp/macro limitation, not a rooting
  artifact. The build is green.

  SAFETY: if the directory layout this expects is missing (no
  shell-macos-mlx/Package.swift beside the client's root), the wrapper is a
  byte-for-byte transparent relay — no rewrite, no injection.

  Threads: stdin (client->server) parses frames until the one-time injection
  completes, then relays raw; stdout (server->client) is always raw relay.
"""

import json
import os
import subprocess
import sys
import threading

REAL_LSP = "/usr/bin/sourcekit-lsp"
ROOT_PACKAGE_DIR = "shell-macos-mlx"  # needs root-class preparation (tests)
ADDED_PACKAGE_DIRS = ["shell-macos"]  # served fine as added workspaces


def frame(body: bytes) -> bytes:
    return b"Content-Length: %d\r\n\r\n%s" % (len(body), body)


class Relay:
    def __init__(self, root_uri: str | None, added_uris: list[str]):
        self.proc = subprocess.Popen(
            [REAL_LSP],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=None,  # inherit: sourcekit-lsp's own stderr stays visible
        )
        self.root_uri = root_uri
        self.added_uris = added_uris
        assert self.proc.stdin is not None and self.proc.stdout is not None
        self.downstream_write_lock = threading.Lock()
        self.injected = False

    # -- client -> server --------------------------------------------------

    def client_loop(self) -> None:
        """Parse client frames until the one-time injection is done, then
        relay raw. Only `initialize` (rewrite) and `initialized` (inject
        after) are ever modified; everything else forwards verbatim."""
        buf = b""
        fd = sys.stdin.buffer
        while True:
            try:
                chunk = os.read(fd.fileno(), 65536)
            except OSError:
                chunk = b""
            if not chunk:
                self.close()
                return
            buf += chunk
            while True:
                parsed = self.take_frame(buf)
                if parsed is None:
                    break
                head, body, buf = parsed
                if self.injected:
                    # Past the injection: relay verbatim, no parsing needed.
                    self.forward(head + body)
                    continue
                try:
                    message = json.loads(body)
                except (ValueError, UnicodeDecodeError):
                    self.forward(head + body)
                    continue
                method = message.get("method")
                if method == "initialize" and self.root_uri is not None:
                    self.rewrite_initialize(message)
                    # Forward the REWRITTEN frame (never also the original).
                    self.forward(frame(json.dumps(message).encode()))
                elif method == "initialized":
                    self.forward(head + body)
                    self.inject_folders()
                    self.injected = True
                else:
                    self.forward(head + body)

    def rewrite_initialize(self, message: dict) -> None:
        params = message.get("params")
        if not isinstance(params, dict):
            params = {}
            message["params"] = params
        params["rootUri"] = self.root_uri
        params.pop("rootPath", None)
        folders = params.get("workspaceFolders")
        if isinstance(folders, list) and folders:
            folders[0] = {"uri": self.root_uri, "name": ROOT_PACKAGE_DIR}

    def inject_folders(self) -> None:
        if not self.added_uris:
            return
        added = [
            {"uri": uri, "name": uri.rstrip("/").rsplit("/", 1)[-1]}
            for uri in self.added_uris
        ]
        inject = {
            "jsonrpc": "2.0",
            "method": "workspace/didChangeWorkspaceFolders",
            "params": {"event": {"added": added, "removed": []}},
        }
        self.forward(frame(json.dumps(inject).encode()))

    def take_frame(self, buf: bytes):
        i = buf.find(b"\r\n\r\n")
        if i < 0:
            return None
        length = None
        for line in buf[:i].decode("latin-1").split("\r\n"):
            if line.lower().startswith("content-length:"):
                length = int(line.split(":", 1)[1].strip())
        if length is None or len(buf) < i + 4 + length:
            return None
        return buf[: i + 4], buf[i + 4 : i + 4 + length], buf[i + 4 + length :]

    def forward(self, data: bytes) -> None:
        stdin = self.proc.stdin
        if stdin is None:
            return
        with self.downstream_write_lock:
            try:
                stdin.write(data)
                stdin.flush()
            except (OSError, ValueError):
                pass

    # -- server -> client --------------------------------------------------

    def server_loop(self) -> None:
        stdout = self.proc.stdout
        if stdout is None:
            self.close()
            return
        try:
            while True:
                chunk = os.read(stdout.fileno(), 65536)
                if not chunk:
                    break
                sys.stdout.buffer.write(chunk)
                sys.stdout.buffer.flush()
        except (OSError, ValueError):
            pass
        self.close()

    def close(self) -> None:
        stdin = self.proc.stdin
        if stdin is None:
            return
        try:
            stdin.close()
        except (OSError, ValueError):
            pass


def main() -> None:
    # The client's root arrives only inside `initialize`; until then we do
    # not know (nor need) the repo root. Resolve it from OUR cwd — Zed sets
    # the wrapper's cwd to the workspace root.
    cwd = os.getcwd()
    root_pkg = os.path.join(cwd, ROOT_PACKAGE_DIR)
    if os.path.isfile(os.path.join(root_pkg, "Package.swift")):
        root_uri = "file://" + root_pkg
        added_uris = []
        for d in ADDED_PACKAGE_DIRS:
            p = os.path.join(cwd, d)
            if os.path.isfile(os.path.join(p, "Package.swift")):
                added_uris.append("file://" + p)
    else:
        # Layout mismatch: pure transparent relay (also covers being run
        # from inside a package dir — root it there honestly).
        if os.path.isfile(os.path.join(cwd, "Package.swift")):
            root_uri = "file://" + cwd
            added_uris = []
        else:
            root_uri = None
            added_uris = []

    relay = Relay(root_uri, added_uris)
    server_thread = threading.Thread(target=relay.server_loop, daemon=True)
    server_thread.start()

    client_thread = threading.Thread(target=relay.client_loop, daemon=True)
    client_thread.start()

    client_thread.join()
    try:
        relay.proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        relay.proc.kill()
    sys.stdout.buffer.flush()


if __name__ == "__main__":
    main()
