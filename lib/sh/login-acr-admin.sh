#!/bin/sh

az acr credential show --name  "$REGISTRY_TENANT" --query "passwords[0].value"

