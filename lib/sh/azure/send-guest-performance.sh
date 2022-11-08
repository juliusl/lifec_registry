#!/bin/sh

az storage blob upload-batch -d "$TENANT" -s "$WORK_DIR/.guest/performance" --overwrite --account-name "$ACCOUNT_NAME"
