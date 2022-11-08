#!/bin/sh

az storage blob upload-batch -d "$TENANT" -s "$WORK_DIR/.guest" --overwrite --account-name "$ACCOUNT_NAME"
rm  "$WORK_DIR/.guest/status/control"
rm  "$WORK_DIR/.guest/status/frames"
rm  "$WORK_DIR/.guest/status/blob"
rm  "$WORK_DIR/.guest/performance/control"
rm  "$WORK_DIR/.guest/performance/frames"
rm  "$WORK_DIR/.guest/performance/blob"
rm  "$WORK_DIR/.guest/journal/control"
rm  "$WORK_DIR/.guest/journal/frames"
rm  "$WORK_DIR/.guest/journal/blob"
