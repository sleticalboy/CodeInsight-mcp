#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

main() {
  ruby - "$ROOT_DIR" <<'RUBY'
root = ARGV.fetch(0)
files = ["README.md", "CHANGELOG.md"] + Dir.glob(File.join(root, "docs", "*.md")).map { |path| path.delete_prefix("#{root}/") }
failures = []

files.sort.each do |relative_file|
  path = File.join(root, relative_file)
  in_fence = false

  File.readlines(path, chomp: true).each_with_index do |line, index|
    if line.match?(/^\s*```/)
      in_fence = !in_fence
      next
    end
    next if in_fence

    line.scan(/\[[^\]]+\]\(([^)]+)\)/).each do |match|
      target = match.first.strip
      next if target.empty?
      next if target.start_with?("#")
      next if target.match?(/\A[a-z][a-z0-9+.-]*:/i)

      link_path = target.split("#", 2).first
      next if link_path.empty?

      decoded_path = link_path.gsub("%20", " ")
      absolute = File.expand_path(decoded_path, File.dirname(path))
      unless absolute.start_with?("#{root}/") || absolute == root
        failures << "#{relative_file}:#{index + 1}: link escapes repository: #{target}"
        next
      end
      next if File.file?(absolute) || File.directory?(absolute)

      failures << "#{relative_file}:#{index + 1}: missing link target: #{target}"
    end
  end
end

if failures.any?
  warn failures.join("\n")
  exit 1
end

puts "docs link smoke passed"
RUBY
}

main "$@"
