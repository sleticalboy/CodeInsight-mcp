#!/usr/bin/env bash

smoke_run_step() {
  local total="$1"
  local index="$2"
  local label="$3"
  shift 3

  echo "[$index/$total] $label"
  "$@"
}
