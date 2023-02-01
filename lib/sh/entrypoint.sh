#!/bin/sh
set -e

apt install /opt/acr/pkg/overlaybd-0.6.0-1.x86_64.deb
apt install /opt/acr/pkg/overlaybd-snapshotter_0.6.0_amd64.deb

/opt/acr/bin/acr mirror start
