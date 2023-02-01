#!/bin/sh

# Install Overlaybd binaries
pushd /tmp || exit 1
wget https://github.com/containerd/overlaybd/releases/download/v0.6.0/overlaybd-0.6.0-1.x86_64.deb
wget https://github.com/containerd/accelerated-container-image/releases/download/v0.6.0/overlaybd-snapshotter_0.6.0_amd64.deb
apt install overlaybd-0.6.0-1.x86_64.deb -y
apt install overlaybd-snapshotter_0.6.0_amd64.deb -y
rm overlaybd-0.6.0-1.x86_64.deb
rm overlaybd-snapshotter_0.6.0_amd64.deb
popd

# Enable kernel feature
modprobe target_core_user

# Edit containerd settings
tee -a /etc/containerd/config.toml > /dev/null <<EOF
version = 2
[plugins.cri]
    [plugins.cri.containerd]
        snapshotter = "overlaybd"
        disable_snapshot_annotations = false
    
    [plugins."io.containerd.grpc.v1.cri".registry]
        config_path = "/etc/containerd/certs.d"
EOF
