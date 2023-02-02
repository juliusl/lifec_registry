#!/bin/sh

set -e

sudo systemctl enable /opt/overlaybd/overlaybd-tcmu.service
sudo systemctl enable /opt/overlaybd/snapshotter/overlaybd-snapshotter.service
sudo systemctl start overlaybd-tcmu
sudo systemctl start overlaybd-snapshotter

sudo systemctl restart containerd
