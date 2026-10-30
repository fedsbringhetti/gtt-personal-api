#!/usr/bin/env python3
"""Claude Code PreToolUse hook: stops `git commit` and `gh pr create` when the docs are behind.

Reads the hook payload (JSON) from stdin. For any other command it does nothing. For a
commit or a PR it blocks (exit code 2, reason on stderr, which Claude reads) when:
- code changed on the branch and no documentation file changed, or
- check_docs.py finds broken links or code excerpts that no longer match the code.

Add "[skip docs]" to the commit message (or the PR command) for a change with no
documentation impact, such as a pure refactor or a dependency bump.
"""

import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from docs_impact import analyze  # noqa: E402

TRIGGER = re.compile(r"\bgit\s+(?:-C\s+\S+\s+)?commit\b|\bgh\s+pr\s+create\b")
SKIP = "[skip docs]"


def main():
    try:
        payload = json.load(sys.stdin)
    except json.JSONDecodeError:
        return 0
    command = (payload.get("tool_input") or {}).get("command") or ""
    if not TRIGGER.search(command) or SKIP in command:
        return 0

    repo = os.environ.get("CLAUDE_PROJECT_DIR") or payload.get("cwd") or os.getcwd()
    report = analyze(repo, "main")
    problems = []

    if report["code_changed"] and not report["docs_changed"]:
        lines = [
            "Code changed on this branch but no documentation was updated.",
            "Run the docs-sync skill (or follow it by hand), then retry. Documents to review:",
        ]
        lines += [f"  - {target}" for target in report["review"]]
        lines.append(f"  - docs/development/: chapter for this step (next number {report['next_chapter_number']})")
        lines.append(f'If this change truly has no documentation impact, add "{SKIP}" to the commit message.')
        problems.append("\n".join(lines))

    check = subprocess.run(
        [sys.executable, "-I", os.path.join(HERE, "check_docs.py"), "--repo", repo],
        capture_output=True,
        text=True,
    )
    if check.returncode != 0:
        problems.append(check.stdout.strip())

    if problems:
        print("\n\n".join(problems), file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
