#!/bin/sh

az storage blob list --container-name "$TENANT" --account-name "$ACCOUNT_NAME"
