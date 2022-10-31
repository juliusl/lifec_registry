#!/bin/sh

user="$REGISTRY_USER:$REGISTRY_TOKEN"
src="$REGISTRY_TENANT.$REGISTRY_HOST/$REGISTRY_REPO:$REFERENCE"
dest="$REGISTRY_TENANT.$REGISTRY_HOST/$REGISTRY_REPO:$REFERENCE-overlaybd"

bin/ctr images pull --user "$user" "$src"
bin/ctr obdconv --user "$user" "$src" "$dest"
bin/ctr images push --user "$user" "$dest"

