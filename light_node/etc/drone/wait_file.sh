#!/usr/bin/env bash
file="$1"
timeout="$2"

wait_file() {
  local file="$1"; shift
  local wait_seconds="${1:-10}"; shift # 10 seconds as default timeout

  until test $((wait_seconds--)) -eq 0 -o -e "$file" ; do sleep 1; done

  ((++wait_seconds))
}

wait_file "$file" $timeout || {
  echo "File '$file' is missing after waiting for $timeout seconds!"
  exit 1
}

echo "OK - File '$file' found"