#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKFLOW_FILE="$ROOT_DIR/.github/workflows/release-build.yml"

fail() {
  echo "release workflow guard smoke failed: $*" >&2
  exit 1
}

main() {
  ruby - "$WORKFLOW_FILE" <<'RUBY'
require "yaml"

workflow_file = ARGV.fetch(0)
workflow = YAML.load_file(workflow_file)

def fail!(message)
  warn("release workflow guard smoke failed: #{message}")
  exit(1)
end

def expect(condition, message)
  fail!(message) unless condition
end

def scalar(value)
  value.to_s.gsub(/\s+/, " ").strip
end

jobs = workflow.fetch("jobs") { fail!("missing jobs") }
permissions = workflow.fetch("permissions") { fail!("missing top-level permissions") }

expect(permissions["actions"] == "read", "top-level actions permission must be read")
expect(permissions["contents"] == "read", "top-level contents permission must be read")

pretag = jobs.fetch("pretag-gate") { fail!("missing pretag-gate job") }
build = jobs.fetch("build") { fail!("missing build job") }
release = jobs.fetch("release") { fail!("missing release job") }
homebrew = jobs.fetch("homebrew-tap-sync") { fail!("missing homebrew-tap-sync job") }

pretag_if = scalar(pretag["if"])
expect(pretag["name"] == "verify-pretag-ci", "pretag-gate job must keep verify-pretag-ci name")
expect(pretag_if == "startsWith(github.ref, 'refs/tags/v')", "pretag-gate must run only for v tags")

pretag_runs = pretag.fetch("steps").map { |step| step["run"].to_s }
pretag_command = pretag_runs.find { |run| run.include?("scripts/release-pretag-check.sh") }
expect(pretag_command, "pretag-gate must run release-pretag-check.sh")
expect(pretag_command.include?("--repo \"${{ github.repository }}\""), "pretag command must bind repository")
expect(pretag_command.include?("--head-sha \"${TAG_SHA}\""), "pretag command must bind tag target SHA")
expect(pretag_command.end_with?(" main"), "pretag command must check main branch CI")

pretag_env = pretag.fetch("steps").find { |step| step["run"].to_s.include?("release-pretag-check.sh") }.fetch("env")
expect(pretag_env["GH_TOKEN"] == "${{ github.token }}", "pretag gate must use github token")
expect(pretag_env["TAG_SHA"] == "${{ github.sha }}", "pretag gate must pass github.sha")

build_if = scalar(build["if"])
expect(build["needs"] == "pretag-gate", "build job must need pretag-gate")
expect(build_if.include?("always()"), "build condition must use always() so manual builds survive skipped pretag")
expect(build_if.include?("github.event_name != 'workflow_dispatch' || inputs.tag == ''"), "build condition must allow manual artifact builds without tag input")
expect(build_if.include?("!startsWith(github.ref, 'refs/tags/v') || needs.pretag-gate.result == 'success'"), "tag builds must require successful pretag-gate")

release_if = scalar(release["if"])
expect(release["needs"] == "build", "release job must need build artifacts")
expect(release_if == "startsWith(github.ref, 'refs/tags/v')", "release job must run only for v tags")
expect(release.fetch("permissions")["actions"] == "read", "release job needs actions read permission to download artifacts")
expect(release.fetch("permissions")["contents"] == "write", "release job needs contents write permission")

release_uses = release.fetch("steps").map { |step| step["uses"].to_s }
expect(release_uses.include?("actions/download-artifact@v8"), "release job must download build artifacts")

homebrew_if = scalar(homebrew["if"])
expect(homebrew_if == "github.event_name == 'workflow_dispatch' && inputs.tag != ''", "homebrew sync must stay workflow_dispatch tag-only")

has_always = build_if.include?("always()")
allows_manual_build = build_if.include?("github.event_name != 'workflow_dispatch' || inputs.tag == ''")
requires_pretag_success_for_tags = build_if.include?("!startsWith(github.ref, 'refs/tags/v') || needs.pretag-gate.result == 'success'")

tag_push_pretag_success = has_always && allows_manual_build && requires_pretag_success_for_tags
tag_push_pretag_failure = has_always && allows_manual_build && !requires_pretag_success_for_tags
manual_build_without_tag_input = has_always && allows_manual_build
manual_homebrew_sync_with_tag_input = allows_manual_build && homebrew_if.include?("inputs.tag == ''")

expect(tag_push_pretag_success, "tag push with pretag success should build")
expect(!tag_push_pretag_failure, "tag push with failed pretag should not build")
expect(manual_build_without_tag_input, "manual artifact build without tag should build")
expect(!manual_homebrew_sync_with_tag_input, "manual Homebrew sync should not build artifacts")

puts "release workflow guard smoke passed"
RUBY
}

main "$@"
