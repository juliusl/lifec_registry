#!/bin/sh

az storage blob download-batch -d "$WORK_DIR/.guest" -s "$TENANT-guest" --account-name "$ACCOUNT_NAME"
