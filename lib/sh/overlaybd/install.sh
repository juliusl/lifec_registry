#!/bin/sh
set -e

# Download Overlaybd binaries
wget https://github.com/containerd/overlaybd/releases/download/v0.6.0/overlaybd-0.6.0-1.x86_64.deb
wget https://github.com/containerd/accelerated-container-image/releases/download/v0.6.0/overlaybd-snapshotter_0.6.0_amd64.deb
sudo apt-get install ./overlaybd-0.6.0-1.x86_64.deb
sudo apt-get install ./overlaybd-snapshotter_0.6.0_amd64.deb
rm ./overlaybd-0.6.0-1.x86_64.deb
rm ./overlaybd-snapshotter_0.6.0_amd64.deb

# Enable kernel feature
sudo modprobe target_core_user

# Enable containerd settings
touch /etc/containerd/config.toml
tee -a /etc/containerd/config.toml > /dev/null <<EOF
version = 2
[plugins."io.containerd.grpc.v1.cri".containerd]
    snapshotter = "overlaybd"
    disable_snapshot_annotations = false
    
[plugins."io.containerd.grpc.v1.cri".registry]
    config_path = "/etc/containerd/certs.d"
EOF
