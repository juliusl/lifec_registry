#!/bin/bash

src="$REGISTRY_NAME.$REGISTRY_HOST/$REPO:$REFERENCE"
dest="$REGISTRY_NAME.$REGISTRY_HOST/$REPO:$REFERENCE-nydus"

bin/nydusify convert --nydus-image "$NYDUS_INSTALL_DIR/nydus-image" --source "$src" --target "$dest"

