#!/usr/bin/env bash
expected="$1"
result="$2"
if [ "${expected}" != "${result}" ]; then
  echo "Unexpected result: \"${result}\""
  echo "Expected: \"${expected}\""
  exit 2
fi
echo "OK"