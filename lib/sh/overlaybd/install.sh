#!/bin/sh
set -e

# Download Overlaybd binaries
wget https://github.com/containerd/overlaybd/releases/download/latest/overlaybd-0.6.0-1.x86_64.deb
wget https://github.com/containerd/accelerated-container-image/releases/download/v0.6.0/overlaybd-snapshotter_0.6.0_amd64.deb
sudo apt-get install ./overlaybd-0.6.0-1.x86_64.deb
sudo apt-get install ./overlaybd-snapshotter_0.6.0_amd64.deb
rm ./overlaybd-0.6.0-1.x86_64.deb
rm ./overlaybd-snapshotter_0.6.0_amd64.deb

# Enable kernel feature
sudo modprobe target_core_user
