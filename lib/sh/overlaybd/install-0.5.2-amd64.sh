#!/bin/bash

# Installs .deb from overlaybd releases

OVERLAYBD_SNAPSHOTTER_DOWNLOAD_LINK='https://github.com/containerd/accelerated-container-image/releases/download/v0.5.2/overlaybd-snapshotter_0.5.2_amd64.deb'
OVERLAYBD_DOWNLOAD_LINK='https://github.com/containerd/overlaybd/releases/download/v0.5.2/overlaybd-0.5.2-1.x86_64.deb'

wget "OVERLAYBD_DOWNLOAD_LINK"
dpkg -i "overlaybd-0.5.2-1.x86_64.deb"

wget "OVERLAYBD_SNAPSHOTTER_DOWNLOAD_LINK"
dpkg -i "overlaybd-snapshotter_0.5.2_amd64.deb"
