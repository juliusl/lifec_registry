#!/bin/sh

az storage blob list --container-name "$TENANT-guest" --account-name "$ACCOUNT_NAME" --query '[].name' 
