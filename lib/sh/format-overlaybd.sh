#!/bin/sh

ctr obdconv --user "$REGISTRY_USER:$REGISTRY_TOKEN" --push-artifact "$REGISTRY_NAME.$REGISTRY_HOST/$REPO:$OBJECT"

