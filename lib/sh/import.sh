#!/bin/sh

user="$REGISTRY_USER:$REGISTRY_TOKEN"
dest="$REGISTRY_TENANT.$REGISTRY_HOST/$REPO:$REFERENCE"

bin/ctr images pull --user "$user" "$SOURCE"
bin/ctr images tag  "$dest"
bin/ctr images push --user "$user" "$dest" 

