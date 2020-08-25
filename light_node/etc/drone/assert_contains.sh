#!/usr/bin/env bash
tested="$1"
searched="$2"
if [[ "${tested}" != *"${searched}"* ]]; then
  echo "Tested: \"${tested}\""
  echo "Not contains: \"${searched}\""
  exit 2
fi
echo "OK"