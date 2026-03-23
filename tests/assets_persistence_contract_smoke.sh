#!/usr/bin/env bash
set -euo pipefail

rg -n "assets.redb" readme.md
rg -n "\\.mica-term-portable" readme.md
rg -n "working directory" readme.md
rg -n "data/" readme.md
