#!/bin/sh

az storage blob upload-batch -d "$TENANT-guest" -s "$WORK_DIR/.guest-commands" --overwrite --account-name "$ACCOUNT_NAME"
rm -rf "$WORK_DIR/.guest-commands"
