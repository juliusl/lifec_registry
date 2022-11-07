#!/bin/sh

az storage blob download-batch -d "$WORK_DIR" -s "$TENANT-guest" --account-name "$ACCOUNT_NAME"
