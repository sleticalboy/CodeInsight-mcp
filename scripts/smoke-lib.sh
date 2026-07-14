#!/usr/bin/env bash

# Shared helper for smoke wrappers; source this file instead of executing it.
smoke_run_step() {
  local total="$1"
  local index="$2"
  local label="$3"
  shift 3

  echo "[$index/$total] $label"
  "$@"
}
