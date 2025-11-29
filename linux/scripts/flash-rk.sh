#!/usr/bin/env bash
set -euo pipefail
IMG="${1:?image path}"
rkdeveloptool wl 0 "$IMG"
rkdeveloptool rd
