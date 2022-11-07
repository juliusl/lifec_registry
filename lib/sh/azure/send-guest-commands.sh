#!/bin/sh

az storage blob upload-batch -d "$TENANT-guest" -s "$WORK_DIR/.guest" --overwrite --account-name "$ACCOUNT_NAME"
