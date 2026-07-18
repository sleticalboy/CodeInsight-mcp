#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

main() {
  ruby - "$ROOT_DIR" <<'RUBY'
root = ARGV.fetch(0)
readme = File.read(File.join(root, "README.md"))
cases = File.read(File.join(root, "docs", "adoption-cases.md"))

def fetch(content, pattern, description)
  match = content.match(pattern)
  unless match
    warn "missing #{description}"
    exit 1
  end
  match[1]
end

repos = fetch(cases, /^- Public repositories: `([^`]+)`$/, "adoption case repo count")
baseline = fetch(cases, /^- Blind first-read baseline: `([^`]+)` source lines$/, "adoption case baseline")
routed = fetch(cases, /^- CodeInsight routed first-read: `([^`]+)` source lines$/, "adoption case routed lines")
avoided = fetch(cases, /^- Source lines avoided before broad file reading: `([^`]+)`$/, "adoption case avoided lines")
reduction = fetch(cases, /^- Aggregate first-read reduction: `([^`]+)`$/, "adoption case aggregate reduction")
read_less = fetch(cases, /^- Aggregate read-less ratio: `([^`]+)`$/, "adoption case read-less ratio")

expected = "The adoption case summary covers #{repos} public repositories and routes a first read\n" \
  "  to #{routed} of #{baseline} source lines, avoiding #{avoided} lines before broad file\n" \
  "  reading, a #{reduction} aggregate reduction and #{read_less} aggregate read-less ratio."

unless readme.include?(expected)
  warn "README adoption summary is out of sync with docs/adoption-cases.md"
  warn "expected:"
  warn expected
  exit 1
end

unless readme.include?("Per-repository adoption metrics, commits, and refresh commands live in")
  warn "README is missing adoption detail handoff sentence"
  exit 1
end

puts "README adoption summary smoke passed"
RUBY
}

main "$@"
