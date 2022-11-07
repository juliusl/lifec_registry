#!/bin/sh

az storage blob upload-batch -d "$TENANT-guest" -s "$WORK_DIR" --account-name "$ACCOUNT_NAME"
