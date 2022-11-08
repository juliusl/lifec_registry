#!/bin/sh

az storage blob download-batch -d "$WORK_DIR/.guest-commands" -s "$TENANT-guest" --account-name "$ACCOUNT_NAME"
az storage blob delete-batch -s "$TENANT-guest" --account-name "$ACCOUNT_NAME"