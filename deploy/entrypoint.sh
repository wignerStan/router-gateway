#!/bin/sh
set -e
exec gateway --config /etc/gateway/gateway.yaml "$@"
