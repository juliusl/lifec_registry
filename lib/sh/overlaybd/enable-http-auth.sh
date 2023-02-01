#!/bin/sh
apt install jq -y

# Edit overlaybd settings
jq '.credentialConfig.mode = "http"' /etc/overlaybd/overlaybd.json > tmp.json
mv tmp.json /etc/overlaybd/overlaybd.json

jq '.credentialConfig.path = "localhost:8567/auth"' /etc/overlaybd/overlaybd.json > tmp.json
mv tmp.json /etc/overlaybd/overlaybd.json

cat /etc/overlaybd/overlaybd.json