#!/usr/bin/env python3
"""Checks the documentation against the repository.

1. Every relative link in the Markdown files points to an existing file, and every
   `#anchor` to an existing heading.
2. Every code excerpt in docs/ still matches the real code. Each significant line of a
   ```rust, ```sql, ```dockerfile, ```yaml, ```graphql, ```ts, ```svelte or ```html block
   must appear in the matching source files, ignoring whitespace (so a call reflowed onto
   one line still matches).
   Lines with "...", comments and very short lines are skipped. A block whose info string
   contains "illustrative" (```sql illustrative) is not checked, and neither are the plans in
   docs/plan/, which describe code before it exists.

Usage: check_docs.py [--repo PATH]
Exit code 0 when everything is fine, 1 with a list of problems otherwise.
"""

import argparse
import glob
import os
import re
import sys

MARKDOWN = ["README.md", "PLAN.md", "CLAUDE.md", "docs/**/*.md"]

# Code block language -> source files it is compared with.
SOURCES = {
    "rust": ["src/**/*.rs", "tests/**/*.rs"],
    "sql": ["migrations/*.sql"],
    "dockerfile": ["Dockerfile"],
    "yaml": ["docker-compose.yml", ".github/workflows/*.yml"],
    "graphql": ["src/upstream/queries.rs"],
    "ts": ["web/src/**/*.ts", "web/*.ts"],
    "svelte": ["web/src/**/*.svelte"],
    "html": ["web/index.html"],
}
COMMENT_PREFIXES = {
    "rust": ("//", "///", "//!"),
    "sql": ("--",),
    "dockerfile": ("#",),
    "yaml": ("#",),
    "graphql": ("#",),
    "ts": ("//", "/*", "*"),
    "svelte": ("//", "/*", "*", "<!--"),
    "html": ("<!--",),
}
MIN_LINE_LEN = 12

FENCE = re.compile(r"^```(\S*)(.*)$")
LINK = re.compile(r"\]\(([^)\s]+)\)")


def files(repo, patterns):
    found = []
    for pattern in patterns:
        found += glob.glob(os.path.join(repo, pattern), recursive=True)
    return sorted(set(found))


def squash(text):
    return re.sub(r"\s+", "", text)


def strip_comments(text, language):
    if language == "sql":
        return re.sub(r"--[^\n]*", "", text)
    if language == "dockerfile":
        text = re.sub(r"\\[ \t]*\n", " ", text)  # line continuations
        return re.sub(r"(?m)^\s*#[^\n]*", "", text)
    if language == "yaml":
        return re.sub(r"(?m)^\s*#[^\n]*", "", text)
    if language in ("rust", "ts", "svelte"):
        return re.sub(r"(?m)\s//[^\n]*$", "", text)  # trailing comments; keeps "http://"
    return text


def anchors(path):
    result = set()
    in_code = False
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            if line.startswith("```"):
                in_code = not in_code
            if in_code:
                continue
            match = re.match(r"^#+\s+(.*)", line)
            if match:
                title = match.group(1).strip().lower()
                result.add(re.sub(r"[^\w\- ]", "", title).replace(" ", "-"))
    return result


def code_blocks(text):
    """Yields (language, info, first_line_number, lines) for each fenced block."""
    lines = text.split("\n")
    i = 0
    while i < len(lines):
        match = FENCE.match(lines[i])
        if match and match.group(1):
            language, info, start = match.group(1).lower(), match.group(2), i + 2
            body = []
            i += 1
            while i < len(lines) and not lines[i].startswith("```"):
                body.append(lines[i])
                i += 1
            yield language, info, start, body
        i += 1


def check_links(repo, path, text, problems):
    without_code = re.sub(r"```.*?```", "", text, flags=re.S)
    for target in LINK.findall(without_code):
        if target.startswith(("http://", "https://", "mailto:")):
            continue
        file_part, _, anchor = target.partition("#")
        resolved = os.path.normpath(os.path.join(os.path.dirname(path), file_part)) if file_part else path
        rel = os.path.relpath(path, repo)
        if not os.path.exists(resolved):
            problems.append(f"{rel}: broken link -> {target}")
        elif anchor and resolved.endswith(".md") and anchor not in anchors(resolved):
            problems.append(f"{rel}: missing anchor -> {target}")


def check_excerpts(repo, path, text, sources, problems):
    rel = os.path.relpath(path, repo)
    for language, info, start, body in code_blocks(text):
        if language not in sources or "illustrative" in info:
            continue
        haystack = sources[language]
        for offset, line in enumerate(body):
            stripped = line.strip()
            if (
                len(stripped) < MIN_LINE_LEN
                or "..." in stripped
                or stripped.startswith(COMMENT_PREFIXES[language])
            ):
                continue
            if language == "dockerfile":
                stripped = stripped.rstrip("\\")  # a continued line in the excerpt
            needle = squash(strip_comments(stripped, language))
            if needle and needle not in haystack:
                problems.append(f"{rel}:{start + offset}: excerpt not found in {language} sources: {stripped}")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--repo", default=os.getcwd())
    args = parser.parse_args()
    repo = os.path.abspath(args.repo)

    sources = {}
    for language, patterns in SOURCES.items():
        content = ""
        for source in files(repo, patterns):
            with open(source, encoding="utf-8") as handle:
                content += strip_comments(handle.read(), language)
        sources[language] = squash(content)

    problems = []
    markdown = files(repo, MARKDOWN)
    for path in markdown:
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
        check_links(repo, path, text, problems)
        rel = os.path.relpath(path, repo)
        if rel.startswith("docs") and not rel.startswith(os.path.join("docs", "plan")):
            check_excerpts(repo, path, text, sources, problems)

    if problems:
        print(f"{len(problems)} documentation problem(s):")
        for problem in problems:
            print(f"  - {problem}")
        return 1
    print(f"docs OK: {len(markdown)} Markdown files, links and code excerpts checked")
    return 0


if __name__ == "__main__":
    sys.exit(main())
