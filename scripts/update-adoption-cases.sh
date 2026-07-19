#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
  cat <<'EOF'
usage: scripts/update-adoption-cases.sh [options] [CASE_DOC ...]

Regenerates docs/adoption-cases.md from checked-in adoption case detail pages.

Options:
  --output PATH   Output Markdown path. Default: docs/adoption-cases.md.
  --check         Verify the output is already up to date.
  -h, --help      Show this help text.
EOF
}

main() {
  local output="$ROOT_DIR/docs/adoption-cases.md"
  local check="false"
  local -a cases=()

  while [ "$#" -gt 0 ]; do
    case "$1" in
      --output)
        [ "$#" -ge 2 ] || {
          echo "update adoption cases failed: --output requires a path" >&2
          exit 1
        }
        output="$2"
        shift 2
        ;;
      --check)
        check="true"
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      -*)
        echo "update adoption cases failed: unknown argument: $1" >&2
        exit 1
        ;;
      *)
        cases+=("$1")
        shift
        ;;
    esac
  done

  if [ "${#cases[@]}" -eq 0 ]; then
    while IFS= read -r path; do
      cases+=("$path")
    done < <(find "$ROOT_DIR/docs" -maxdepth 1 -name 'adoption-case-*.md' | sort)
  fi

  if [ "${#cases[@]}" -eq 0 ]; then
    echo "update adoption cases failed: no adoption case docs found" >&2
    exit 1
  fi

  local generated
  generated="$(mktemp)"
  ruby - "$ROOT_DIR" "$generated" "${cases[@]}" <<'RUBY'
root = ARGV.fetch(0)
output = ARGV.fetch(1)
case_paths = ARGV.drop(2)

def fail_parse(path, message)
  warn "#{path}: #{message}"
  exit 1
end

def match_required(content, pattern, path, description)
  match = content.match(pattern)
  fail_parse(path, "missing #{description}") unless match
  match[1]
end

def metric_required(content, label, path)
  pattern = /^\| #{Regexp.escape(label)} \| `([^`]+)`(?: source lines)? \|$/
  match_required(content, pattern, path, label)
end

def number_value(value)
  value.gsub(",", "").to_i
end

def format_number(value)
  value.to_s.reverse.gsub(/(\d{3})(?=\d)/, '\\1,').reverse
end

def ecosystem_for(title, repo)
  return "JavaScript web framework" if title == "Express" || repo.include?("expressjs/express")
  return "Go web framework" if title == "Gin" || repo.include?("gin-gonic/gin")
  return "Rust search library" if title == "Memchr" || repo.include?("BurntSushi/memchr")
  return "Python HTTP library" if title == "Requests" || repo.include?("psf/requests")

  "Public repository"
end

cases = case_paths.map do |path|
  absolute = File.expand_path(path, root)
  relative = absolute.delete_prefix("#{root}/")
  content = File.read(absolute)
  full_title = match_required(content, /^# (.+ Adoption Comparison)$/, relative, "title")
  title = full_title.sub(/ Adoption Comparison\z/, "")
  case_name = File.basename(relative).sub(/\Aadoption-case-/, "").sub(/\.md\z/, "")
  repo = match_required(content, /^- Repository: `([^`]+)`$/, relative, "repository")
  commit = match_required(content, /^- Commit: `([^`]+)`$/, relative, "commit")
  task = match_required(content, /^- Task: `([^`]+)`$/, relative, "task")
  blind = number_value(metric_required(content, "Blind first-read baseline", relative))
  routed = number_value(metric_required(content, "CodeInsight routed first-read", relative))
  avoided = number_value(metric_required(content, "Source lines avoided", relative))
  reduction = metric_required(content, "First-read reduction", relative)
  read_less = metric_required(content, "Read less", relative)
  selected_files = number_value(metric_required(content, "Selected files", relative))
  selected_ranges = number_value(metric_required(content, "Selected ranges", relative))
  tokens = number_value(metric_required(content, "Estimated tokens", relative))
  impacted = number_value(metric_required(content, "Impacted files", relative))
  seed_strategy = match_required(content, /^\| Seed strategy \| `([^`]+)` \|$/, relative, "seed strategy")
  first_file = match_required(content, /^\| First selected file \| `([^`]+)` \|$/, relative, "first selected file")
  companion = match_required(content, /^\| Companion entrypoint \| `([^`]+)` \|$/, relative, "companion entrypoint")
  first_focus = match_required(content, /^\| First reading focus \| ([^|]+) \|$/, relative, "first reading focus").strip
  first_tool = match_required(content, /^\| First suggested tool \| `([^`]+)` \|$/, relative, "first suggested tool")
  risk = match_required(content, /^\| Impact risk \| `([^`]+)` \|$/, relative, "impact risk")

  {
    title: title,
    case_name: case_name,
    ecosystem: ecosystem_for(title, repo),
    task: task,
    blind: blind,
    routed: routed,
    avoided: avoided,
    reduction: reduction,
    read_less: read_less,
    selected_files: selected_files,
    selected_ranges: selected_ranges,
    tokens: tokens,
    impacted: impacted,
    detail: File.basename(relative),
    commit: commit,
    seed_strategy: seed_strategy,
    first_file: first_file,
    companion: companion,
    first_focus: first_focus,
    first_tool: first_tool,
    risk: risk
  }
end

cases.sort_by! { |entry| entry[:title] }
total_blind = cases.sum { |entry| entry[:blind] }
total_routed = cases.sum { |entry| entry[:routed] }
total_avoided = cases.sum { |entry| entry[:avoided] }
total_selected_files = cases.sum { |entry| entry[:selected_files] }
total_selected_ranges = cases.sum { |entry| entry[:selected_ranges] }
total_tokens = cases.sum { |entry| entry[:tokens] }
total_impacted = cases.sum { |entry| entry[:impacted] }
aggregate_reduction = format("%.1f%%", total_avoided * 100.0 / total_blind)
aggregate_read_less = format("%.1fx", total_blind.to_f / total_routed)

summary_rows = cases.map do |entry|
  "| #{entry[:title]} | #{entry[:ecosystem]} | #{entry[:task]} | `#{format_number(entry[:blind])}` | `#{format_number(entry[:routed])}` | `#{format_number(entry[:avoided])}` | `#{entry[:reduction]}` | `#{entry[:read_less]}` | [case](#{entry[:detail]}) |"
end

route_rows = cases.map do |entry|
  "| #{entry[:title]} | `#{entry[:commit]}` | `#{entry[:seed_strategy]}` | `#{entry[:first_file]}` | #{entry[:first_focus]} | `#{entry[:companion]}` | `#{entry[:first_tool]}` | `#{entry[:risk]}` |"
end

refresh_commands = cases.map do |entry|
  "scripts/update-adoption-case.sh #{entry[:case_name]}"
end

File.write(output, <<~MARKDOWN)
  # Adoption Cases

  This page summarizes checked-in public repository adoption cases for
  CodeInsight as a local-first AI-agent code context router. Each case compares a
  blind first read of all indexed source lines with the first context pack selected
  by `index_project -> project_overview -> context_pack -> impact_analysis`.

  These cases are adoption evidence, not controlled performance benchmarks. They
  show what an AI coding agent can read first before opening files broadly.

  ## Summary

  | Case | Ecosystem | Task | Blind lines | Routed lines | Avoided lines | Reduction | Read less | Details |
  | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
  #{summary_rows.join("\n")}

  Aggregate snapshot:

  - Public repositories: `#{cases.length}`
  - Blind first-read baseline: `#{format_number(total_blind)}` source lines
  - CodeInsight routed first-read: `#{format_number(total_routed)}` source lines
  - Source lines avoided before broad file reading: `#{format_number(total_avoided)}`
  - Aggregate first-read reduction: `#{aggregate_reduction}`
  - Aggregate read-less ratio: `#{aggregate_read_less}`
  - Selected files: `#{format_number(total_selected_files)}`
  - Selected ranges: `#{format_number(total_selected_ranges)}`
  - Estimated tokens: `#{format_number(total_tokens)}`
  - Impacted files reported before edits: `#{format_number(total_impacted)}`

  ## How To Read These Numbers

  The baseline is the number of indexed source lines an agent could read if it
  opened the repository broadly before forming a plan. The routed first-read
  count is the source text selected by CodeInsight for the same task under the
  token budget before broad file reading starts. The reduction and read-less
  ratio describe first-read context routing, not runtime performance, parser
  accuracy, or a claim that unselected code is irrelevant.

  Use these cases as adoption evidence for agent workflow cost and focus. Final
  code conclusions still need normal local verification with the IDE, LSP,
  compiler, test runner, and language-specific tools.

  ## Route Evidence

  | Case | Commit | Seed strategy | First selected file | First reading focus | Companion entrypoint | First suggested tool | Impact risk |
  | --- | --- | --- | --- | --- | --- | --- | --- |
  #{route_rows.join("\n")}

  ## Refresh

  Refresh checked-in snapshots:

  ```bash
  #{refresh_commands.join("\n")}
  ```

  Generate the same shape for another repository:

  ```bash
  scripts/adoption-comparison.sh /path/to/repo \\
    --task "understand the app entrypoint" \\
    --output-dir /tmp/codeinsight-adoption-comparison
  ```

MARKDOWN
RUBY

  if [ "$check" = "true" ]; then
    if ! cmp -s "$generated" "$output"; then
      echo "adoption cases summary is out of date: $output" >&2
      diff -u "$output" "$generated" >&2 || true
      rm -f "$generated"
      exit 1
    fi
    rm -f "$generated"
    echo "adoption cases summary is up to date"
    return
  fi

  mkdir -p "$(dirname "$output")"
  mv "$generated" "$output"
  echo "updated adoption cases summary: $output"
}

main "$@"
