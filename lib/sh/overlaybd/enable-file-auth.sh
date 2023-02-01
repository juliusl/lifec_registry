#!/bin/sh
apt install jq -y

# Edit overlaybd settings
jq '.credentialConfig.mode = "file"' /etc/overlaybd/overlaybd.json > tmp.json
mv tmp.json /etc/overlaybd/overlaybd.json

jq '.credentialConfig.path = "/opt/overlaybd/cred.json"' /etc/overlaybd/overlaybd.json > tmp.json
mv tmp.json /etc/overlaybd/overlaybd.json

cat /etc/overlaybd/overlaybd.json