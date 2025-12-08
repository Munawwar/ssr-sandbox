#!/usr/bin/env python3
"""
SSR Sandbox Python Client

Example usage of the ssr-sandbox in server mode.
Keeps the process alive for fast subsequent renders.
"""

import subprocess
import time
from dataclasses import dataclass
from typing import Optional
import json


@dataclass
class RenderResult:
    ok: bool
    body: str  # HTML if ok, error message if not


class SSRSandbox:
    """Client for ssr-sandbox server mode."""

    def __init__(self, binary_path: str, chunks_dir: str):
        self.binary_path = binary_path
        self.chunks_dir = chunks_dir
        self.process: Optional[subprocess.Popen] = None

    def start(self):
        """Start the ssr-sandbox process."""
        self.process = subprocess.Popen(
            [self.binary_path, "--server", self.chunks_dir],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,  # Line buffered
        )
        # Wait for ready signal (optional, just for cleaner startup)
        # The process writes to stderr when ready

    def stop(self):
        """Stop the ssr-sandbox process."""
        if self.process:
            self.process.stdin.close()
            self.process.wait()
            self.process = None

    def render(self, entry: str, props: dict) -> RenderResult:
        """
        Render a page.

        Args:
            entry: Entry point filename (relative to chunks_dir)
            props: Props to pass to the render function

        Returns:
            RenderResult with ok=True and HTML, or ok=False and error message
        """
        if not self.process:
            raise RuntimeError("SSRSandbox not started. Call start() first.")

        # Send request (2 lines)
        self.process.stdin.write(f"{entry}\n")
        self.process.stdin.write(f"{json.dumps(props)}\n")
        self.process.stdin.flush()

        # Read response header (2 lines)
        status_line = self.process.stdout.readline().strip()
        length_line = self.process.stdout.readline().strip()
        _empty_line = self.process.stdout.readline()  # Empty separator

        # Parse headers
        status = status_line.split(":", 1)[1] if ":" in status_line else "Error"
        length = int(length_line.split(":", 1)[1]) if ":" in length_line else 0

        # Read body
        body = self.process.stdout.read(length)

        return RenderResult(ok=(status == "Ok"), body=body)

    def __enter__(self):
        self.start()
        return self

    def __exit__(self, *args):
        self.stop()


def main():
    """Example usage with timing."""

    # Path to the built binary and chunks
    binary = "./target/release/ssr-sandbox"
    chunks = "./example-dist"

    with SSRSandbox(binary, chunks) as ssr:
        # First render - includes cold start
        start = time.perf_counter()
        result = ssr.render("entry.js", {"page": "home", "title": "My App"})
        first_time = (time.perf_counter() - start) * 1000

        print(f"First render:  {first_time:.2f}ms (includes V8 cold start)")
        print(f"Status: {'OK' if result.ok else 'ERROR'}")
        print(f"HTML length: {len(result.body)} bytes")
        print()

        # Subsequent renders - V8 already warm
        times = []
        for i in range(10):
            page = ["home", "about", "contact"][i % 3]
            start = time.perf_counter()
            result = ssr.render("entry.js", {"page": page})
            elapsed = (time.perf_counter() - start) * 1000
            times.append(elapsed)

        avg_time = sum(times) / len(times)
        print(f"Subsequent renders (avg of 10): {avg_time:.2f}ms")
        print(f"Min: {min(times):.2f}ms, Max: {max(times):.2f}ms")
        print()

        # Show sample output
        print("Sample HTML (first 500 chars):")
        print("-" * 40)
        print(result.body[:500])
        print("-" * 40)


if __name__ == "__main__":
    main()
