#!/usr/bin/env bash
set -euo pipefail

PROVIDER="codebase-memory-mcp"
ROOT_PATH=""
OUTPUT=""
CANDIDATE_LIMIT=10
CONFIDENCE=""
TEMP_DIR=""

SEARCH_GRAPH_JSONS=()
SEARCH_CODE_JSONS=()
CODE_SNIPPET_JSONS=()
QUERY_GRAPH_JSONS=()
TRACE_PATH_JSONS=()
ARCHITECTURE_JSONS=()
NOTES=()

fail() {
  echo "codebase-memory backend evidence failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

usage() {
  cat <<'EOF'
usage: scripts/codebase-memory-backend-evidence.sh [options]

Build CodeInsight agent_route backend_evidence JSON from exported
codebase-memory-mcp tool responses. Raw payloads, structuredContent responses,
and JSON-RPC result/content wrappers are accepted.

Options:
  --search-graph-json PATH   JSON response from codebase-memory search_graph.
  --search-code-json PATH    JSON response from codebase-memory search_code.
  --code-snippet-json PATH   JSON response from codebase-memory get_code_snippet.
  --query-graph-json PATH    JSON response from codebase-memory query_graph.
  --trace-path-json PATH     JSON response from codebase-memory trace_path.
  --architecture-json PATH   JSON response from codebase-memory get_architecture.
  --root PATH                Repository root; absolute paths under it become relative.
  --provider NAME            Evidence provider name. Default: codebase-memory-mcp.
  --candidate-limit N        Maximum unique candidate files (1-16). Default: 10.
  --confidence NUMBER        Optional backend confidence value.
  --note TEXT                Add an advisory note. Can be repeated.
  --output PATH              Write JSON to PATH instead of stdout.
  -h, --help                 Show this help text.

Example:
  scripts/codebase-memory-backend-evidence.sh \
    --root /path/to/repo \
    --search-graph-json /tmp/search-graph.json \
    --architecture-json /tmp/architecture.json \
    --output /tmp/codeinsight-backend-evidence.json
EOF
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --search-graph-json)
        [ "$#" -ge 2 ] || fail "--search-graph-json requires a path"
        SEARCH_GRAPH_JSONS+=("$2")
        shift 2
        ;;
      --search-code-json)
        [ "$#" -ge 2 ] || fail "--search-code-json requires a path"
        SEARCH_CODE_JSONS+=("$2")
        shift 2
        ;;
      --code-snippet-json)
        [ "$#" -ge 2 ] || fail "--code-snippet-json requires a path"
        CODE_SNIPPET_JSONS+=("$2")
        shift 2
        ;;
      --query-graph-json)
        [ "$#" -ge 2 ] || fail "--query-graph-json requires a path"
        QUERY_GRAPH_JSONS+=("$2")
        shift 2
        ;;
      --trace-path-json)
        [ "$#" -ge 2 ] || fail "--trace-path-json requires a path"
        TRACE_PATH_JSONS+=("$2")
        shift 2
        ;;
      --architecture-json)
        [ "$#" -ge 2 ] || fail "--architecture-json requires a path"
        ARCHITECTURE_JSONS+=("$2")
        shift 2
        ;;
      --root)
        [ "$#" -ge 2 ] || fail "--root requires a path"
        ROOT_PATH="$2"
        shift 2
        ;;
      --provider)
        [ "$#" -ge 2 ] || fail "--provider requires a name"
        PROVIDER="$2"
        shift 2
        ;;
      --candidate-limit)
        [ "$#" -ge 2 ] || fail "--candidate-limit requires a number"
        CANDIDATE_LIMIT="$2"
        shift 2
        ;;
      --confidence)
        [ "$#" -ge 2 ] || fail "--confidence requires a number"
        CONFIDENCE="$2"
        shift 2
        ;;
      --note)
        [ "$#" -ge 2 ] || fail "--note requires text"
        NOTES+=("$2")
        shift 2
        ;;
      --output)
        [ "$#" -ge 2 ] || fail "--output requires a path"
        OUTPUT="$2"
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        fail "unknown argument: $1"
        ;;
    esac
  done
}

validate_args() {
  require_command jq

  case "$CANDIDATE_LIMIT" in
    ''|*[!0-9]*) fail "--candidate-limit must be a positive integer" ;;
  esac
  [ "$CANDIDATE_LIMIT" -gt 0 ] || fail "--candidate-limit must be > 0"
  [ "$CANDIDATE_LIMIT" -le 16 ] || fail "--candidate-limit must be <= 16"

  if [ -n "$CONFIDENCE" ]; then
    jq -n --arg value "$CONFIDENCE" '$value | tonumber' >/dev/null 2>&1 ||
      fail "--confidence must be numeric"
  fi

  if [ -n "$ROOT_PATH" ]; then
    ROOT_PATH="$(cd "$ROOT_PATH" && pwd)"
  fi

  local input_count=0
  input_count=$((input_count + ${#SEARCH_GRAPH_JSONS[@]}))
  input_count=$((input_count + ${#SEARCH_CODE_JSONS[@]}))
  input_count=$((input_count + ${#CODE_SNIPPET_JSONS[@]}))
  input_count=$((input_count + ${#QUERY_GRAPH_JSONS[@]}))
  input_count=$((input_count + ${#TRACE_PATH_JSONS[@]}))
  input_count=$((input_count + ${#ARCHITECTURE_JSONS[@]}))
  local file
  for file in ${TRACE_PATH_JSONS[@]+"${TRACE_PATH_JSONS[@]}"}; do
    [ -f "$file" ] || fail "input JSON does not exist: $file"
    jq empty "$file" >/dev/null || fail "invalid JSON: $file"
  done
  [ "$input_count" -gt 0 ] || fail "provide at least one exported codebase-memory JSON file"
}

normalize_file() {
  local file="$1"

  file="${file#file://}"
  if [ -n "$ROOT_PATH" ]; then
    case "$file" in
      "$ROOT_PATH"/*) file="${file#"$ROOT_PATH"/}" ;;
    esac
  fi
  file="${file#./}"
  printf '%s\n' "$file"
}

append_candidate() {
  local source="$1"
  local file="$2"
  local symbol="${3:-}"
  local score="${4:-}"
  local reason="${5:-}"
  local normalized

  [ -n "$file" ] || return
  normalized="$(normalize_file "$file")"
  [ -n "$normalized" ] || return
  symbol="${symbol//$'\t'/ }"
  symbol="${symbol//$'\n'/ }"
  reason="${reason//$'\t'/ }"
  reason="${reason//$'\n'/ }"
  printf '%s\t%s\t%s\t%s\t%s\n' \
    "$source" "$normalized" "$symbol" "$score" "$reason" >>"$TEMP_DIR/candidates.tsv"
}

normalized_tool_payload() {
  local source="$1"
  local file="$2"
  local normalized

  [ -f "$file" ] || fail "input JSON does not exist: $file"
  jq empty "$file" >/dev/null || fail "invalid JSON: $file"
  normalized="$(mktemp "$TEMP_DIR/${source}.XXXXXX.json")"
  if ! jq '
    def tool_text:
      [.content[]? | select(.type == "text") | .text] | first;
    def tool_json:
      [.content[]? | select(.type == "text") | .text | fromjson?] | first;
    def unwrap:
      if type != "object" then
        error("tool response must be a JSON object")
      elif .error != null then
        error(.error.message // .error // "JSON-RPC error")
      elif .isError == true then
        error(tool_text // "MCP tool error")
      elif (.structuredContent | type) == "object" then
        .structuredContent
      elif has("result") then
        .result | unwrap
      elif (.content | type) == "array" then
        tool_json // error("MCP response has no JSON text content")
      else
        .
      end;
    unwrap
  ' "$file" >"$normalized"; then
    fail "could not unwrap $source response: $file"
  fi
  printf '%s\n' "$normalized"
}

append_candidates_from_query() {
  local source="$1"
  local file="$2"
  local query="$3"
  local record candidate symbol score reason

  [ -f "$file" ] || fail "input JSON does not exist: $file"
  jq empty "$file" >/dev/null || fail "invalid JSON: $file"

  while IFS= read -r record; do
    candidate="$(jq -r '.file // empty' <<<"$record")"
    symbol="$(jq -r '.symbol // empty' <<<"$record")"
    score="$(jq -r '(.score // empty) | tostring' <<<"$record")"
    reason="$(jq -r '.reason // empty' <<<"$record")"
    append_candidate "$source" "$candidate" "$symbol" "$score" "$reason"
  done < <(jq -c "$query" "$file")
}

append_latency() {
  local file="$1"

  jq -r '(.elapsed_ms // .duration_ms // 0) | numbers' "$file" >>"$TEMP_DIR/latency.txt"
}

collect_candidates() {
  : >"$TEMP_DIR/candidates.tsv"
  : >"$TEMP_DIR/latency.txt"

  local file
  for file in ${CODE_SNIPPET_JSONS[@]+"${CODE_SNIPPET_JSONS[@]}"}; do
    local payload
    payload="$(normalized_tool_payload "get-code-snippet" "$file")"
    append_candidates_from_query "get_code_snippet" "$payload" '
      {
        file: (.file_path // .file // empty),
        symbol: (.qualified_name // .name // null),
        score: null,
        reason: ("get_code_snippet " + (.label // "snippet" | tostring))
      }
      | select(.file != "")
    '
    append_latency "$payload"
  done

  for file in ${SEARCH_GRAPH_JSONS[@]+"${SEARCH_GRAPH_JSONS[@]}"}; do
    local payload
    payload="$(normalized_tool_payload "search-graph" "$file")"
    append_candidates_from_query "search_graph" "$payload" '
      .results[]?
      | {
          file: (.file_path // .file // empty),
          symbol: (.name // .node // .qualified_name // null),
          score: (.score // .similarity // null),
          reason: ("search_graph " + (.label // "result" | tostring))
        }
      | select(.file != "")
    '
    append_latency "$payload"
  done

  for file in ${SEARCH_CODE_JSONS[@]+"${SEARCH_CODE_JSONS[@]}"}; do
    local payload
    payload="$(normalized_tool_payload "search-code" "$file")"
    append_candidates_from_query "search_code" "$payload" '
      .results[]?
      | {
          file: (.file // .file_path // empty),
          symbol: (.node // .name // .qualified_name // null),
          score: (.score // .similarity // null),
          reason: ("search_code " + (.label // "result" | tostring))
        }
      | select(.file != "")
    '
    append_latency "$payload"
  done

  for file in ${QUERY_GRAPH_JSONS[@]+"${QUERY_GRAPH_JSONS[@]}"}; do
    local payload
    payload="$(normalized_tool_payload "query-graph" "$file")"
    append_candidates_from_query "query_graph" "$payload" '
      .columns as $columns
      | ($columns | map(split(".") | last) | index("file_path")) as $file_path_index
      | ($columns | map(split(".") | last) | index("file")) as $file_index
      | ($columns | map(split(".") | last) | index("name")) as $name_index
      | ($columns | map(split(".") | last) | index("qualified_name")) as $qualified_name_index
      | (($file_path_index // $file_index)) as $candidate_file_index
      | select($candidate_file_index != null)
      | .rows[]?
      | {
          file: (.[ $candidate_file_index ] // empty),
          symbol: (if $name_index != null then .[$name_index] elif $qualified_name_index != null then .[$qualified_name_index] else null end),
          score: null,
          reason: "query_graph row"
        }
      | select(.file != "")
    '
    append_latency "$payload"
  done

  for file in ${ARCHITECTURE_JSONS[@]+"${ARCHITECTURE_JSONS[@]}"}; do
    local payload
    payload="$(normalized_tool_payload "get-architecture" "$file")"
    append_candidates_from_query "get_architecture:entry_points" "$payload" '
      .entry_points[]?
      | {
          file: (.file // .file_path // empty),
          symbol: (.name // .qualified_name // null),
          score: (.confidence // .score // null),
          reason: "get_architecture entry point"
        }
      | select(.file != "")
    '
    append_latency "$payload"
  done

  if [ ! -s "$TEMP_DIR/candidates.tsv" ] && [ "${#TRACE_PATH_JSONS[@]}" -eq 0 ]; then
    fail "no candidate files found in exported JSON"
  fi
}

json_string_array_from_lines() {
  jq -R 'select(length > 0)' | jq -s .
}

write_output() {
  local candidate_files_json candidates_json evidence_sources_json notes_json latency_ms evidence_count trace_path_json

  candidate_files_json="$(
    awk -F $'\t' '!seen[$2]++ { print $2 }' "$TEMP_DIR/candidates.tsv" |
      head -n "$CANDIDATE_LIMIT" |
      json_string_array_from_lines
  )"
  candidates_json="$(
    awk -F $'\t' '!seen[$2]++ { print }' "$TEMP_DIR/candidates.tsv" |
      head -n "$CANDIDATE_LIMIT" |
      jq -R '
        split("\t")
        | {
            source: .[0],
            file: .[1],
            evidence: [.[0]]
          }
          + (if (.[2] // "") != "" then {symbol: .[2]} else {} end)
          + (if (.[3] // "") != "" then {score: (.[3] | tonumber)} else {} end)
          + (if (.[4] // "") != "" then {reason: .[4]} else {} end)
      ' |
      jq -s .
  )"
  evidence_sources_json="$(
    awk -F $'\t' '{ print $1 }' "$TEMP_DIR/candidates.tsv" |
      sort -u |
      json_string_array_from_lines
  )"
  evidence_count="$(wc -l <"$TEMP_DIR/candidates.tsv" | tr -d ' ')"
  latency_ms="$(awk '{ sum += $1 } END { print sum + 0 }' "$TEMP_DIR/latency.txt")"

  {
    printf '%s\n' "normalized from exported codebase-memory-mcp JSON"
    awk -F $'\t' '{ count[$1]++ } END { for (source in count) printf "%s contributed %d candidate signal(s)\n", source, count[source] }' \
      "$TEMP_DIR/candidates.tsv" | sort
    if [ "${#NOTES[@]}" -gt 0 ]; then
      printf '%s\n' "${NOTES[@]}"
    fi
  } >"$TEMP_DIR/notes.txt"
  notes_json="$(json_string_array_from_lines <"$TEMP_DIR/notes.txt")"
  trace_path_json="null"
  if [ "${#TRACE_PATH_JSONS[@]}" -gt 0 ]; then
    trace_path_json="$(jq -s 'if length == 1 then .[0] else . end' "${TRACE_PATH_JSONS[@]}")"
  fi

  local jq_args=(
    -n
    --arg provider "$PROVIDER"
    --argjson candidate_files "$candidate_files_json"
    --argjson candidates "$candidates_json"
    --argjson evidence_sources "$evidence_sources_json"
    --argjson evidence_count "$evidence_count"
    --argjson latency_ms "$latency_ms"
    --argjson notes "$notes_json"
    --argjson trace_path "$trace_path_json"
  )
  local jq_program='
    {
      provider: $provider,
      candidate_files: $candidate_files,
      candidates: $candidates,
      evidence_sources: $evidence_sources,
      evidence_count: $evidence_count,
      notes: $notes
    }
    + (if $latency_ms > 0 then {latency_ms: $latency_ms} else {} end)
    + (if $trace_path != null then {tool_results: {trace_path: $trace_path}} else {} end)
  '

  if [ -n "$CONFIDENCE" ]; then
    jq_args+=(--argjson confidence "$CONFIDENCE")
    jq_program+=' + {confidence: $confidence}'
  fi

  if [ -n "$OUTPUT" ]; then
    mkdir -p "$(dirname "$OUTPUT")"
    jq "${jq_args[@]}" "$jq_program" >"$OUTPUT"
  else
    jq "${jq_args[@]}" "$jq_program"
  fi
}

main() {
  parse_args "$@"
  validate_args

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  collect_candidates
  write_output
}

main "$@"
