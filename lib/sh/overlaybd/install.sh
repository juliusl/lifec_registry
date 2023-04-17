#!/bin/sh
set -e

# Download Overlaybd binaries
wget https://github.com/containerd/overlaybd/releases/download/latest/overlaybd-0.6.1-0ubuntu1.22.04.x86_64.deb
wget https://github.com/containerd/accelerated-container-image/releases/download/latest/overlaybd-snapshotter_0.6.1_amd64.deb
sudo apt-get install ./overlaybd-0.6.1-0ubuntu1.22.04.x86_64.deb
sudo apt-get install ./overlaybd-snapshotter_0.6.1_amd64.deb
rm ./overlaybd-0.6.1-0ubuntu1.22.04.x86_64.deb
rm ./overlaybd-snapshotter_0.6.1_amd64.deb
