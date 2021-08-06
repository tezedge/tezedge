#!/usr/bin/env bash

rm -f ../target/fuzz/deps/*.gcda
kill -SIGUSR2 $(pidof light-node)
rm -f *.gcov
sleep 1
rust-cov gcov -p ../target/fuzz/deps/*.gcno
