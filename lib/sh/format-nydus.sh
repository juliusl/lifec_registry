#!/bin/bash

src="$REGISTRY_NAME.$REGISTRY_HOST/$REGISTRY_REPO:$REFERENCE"
dest="$REGISTRY_NAME.$REGISTRY_HOST/$REGISTRY_REPO:$REFERENCE-nydus"

bin/nydusify convert --nydus-image "$NYDUS_INSTALL_DIR/nydus-image" --source "$src" --target "$dest"

