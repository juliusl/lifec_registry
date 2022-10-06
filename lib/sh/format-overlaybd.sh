#!/bin/sh

user="$REGISTRY_USER:$REGISTRY_TOKEN"
src="$REGISTRY_NAME.$REGISTRY_HOST/$REPO:$REFERENCE"
dest="$REGISTRY_NAME.$REGISTRY_HOST/$REPO:$REFERENCE-obd"

bin/ctr images pull --user "$user" "$src"
bin/ctr obdconv --user "$user" "$src" "$dest"
bin/ctr images push --user "$user" "$dest"

