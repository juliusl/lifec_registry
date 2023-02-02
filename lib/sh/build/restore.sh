#!/bin/sh

apt-get update
apt-get install build-essential pkg-config libssl-dev

cargo install cargo-deb
