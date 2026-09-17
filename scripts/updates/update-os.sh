#!/usr/bin/env bash
# Run by controlcenterd-updates@os.service, as root, on-demand.
# set -e gives us "stop at the first failure".
set -euo pipefail

mkdir -p /run/controlcenterd
exec >/run/controlcenterd/update-os.log 2>&1

# No terminal attached -- avoid anything prompting and hanging forever.
export DEBIAN_FRONTEND=noninteractive
export NEEDRESTART_MODE=a

apt-get update
apt-get -y -o Dpkg::Options::="--force-confdef" -o Dpkg::Options::="--force-confold" upgrade
