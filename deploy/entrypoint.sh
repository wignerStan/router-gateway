#!/bin/sh
set -e
export GATEWAY_CONFIG=/etc/gateway/gateway.yaml
exec gateway "$@"
