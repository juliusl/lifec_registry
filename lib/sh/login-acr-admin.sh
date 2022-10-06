#!/bin/bash

az acr credential show --name  "$REGISTRY_NAME" --query "passwords[0].value" > admin_pass

