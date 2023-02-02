#!/bin/sh

az acr login --expose-token --name "$REGISTRY_TENANT" --output tsv --query 'accessToken'

