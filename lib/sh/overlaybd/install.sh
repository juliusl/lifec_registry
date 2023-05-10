#!/bin/sh
set -e

OVERLAYBD_VERSION=0.6.5
OVERLAYBD_SNAPSHOTTER_VERSION=0.6.2
UBUNTU_RELEASE=22.04
# Download Overlaybd binaries 
wget "https://github.com/containerd/overlaybd/releases/download/v${OVERLAYBD_VERSION}/overlaybd-${OVERLAYBD_VERSION}-0ubuntu1.${UBUNTU_RELEASE}.x86_64.deb"
wget "https://github.com/containerd/accelerated-container-image/releases/download/latest/overlaybd-snapshotter_${OVERLAYBD_SNAPSHOTTER_VERSION}_amd64.deb" # TODO: needs to also suppport ARM64
sudo apt-get install "./overlaybd-${OVERLAYBD_VERSION}S-0ubuntu1.${UBUNTU_RELEASE}.x86_64.deb"
sudo apt-get install "./overlaybd-snapshotter_${OVERLAYBD_SNAPSHOTTER_VERSION}_amd64.deb"
rm ./overlaybd-${OVERLAYBD_VERSION}-0ubuntu1.${UBUNTU_RELEASE}.x86_64.deb
rm ./overlaybd-snapshotter_${OVERLAYBD_SNAPSHOTTER_VERSION}_amd64.deb
