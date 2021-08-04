#!/bin/sh
file="$1"
timeout="${2:-15}" # 15 seconds as default timeout
duration_in_sec=0

wait_file() {
  local file="$1";
  local wait_seconds="$2";

  until test $wait_seconds -eq 0 -o -e "$file" ; do
    sleep 1;
    wait_seconds=$((wait_seconds - 1))
  done

  duration_in_sec=$(($2 - wait_seconds))
}

wait_file "$file" $timeout || {
  echo "File '$file' is missing after waiting for $timeout seconds!"
  exit 1
}

echo "OK - File '$file' found in $duration_in_sec seconds!"
