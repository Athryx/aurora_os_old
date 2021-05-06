#!/bin/sh

cd $(dirname $0)/src
grep -v '^\s*$' $(tree -fi | grep -E '.*\.rs$|.*\.asm$' | grep -v '/old/' | grep -v 'build\.rs') | wc -l
