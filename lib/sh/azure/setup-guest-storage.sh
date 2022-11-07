#!/bin/sh

 az storage container create -n "$TENANT" --account-name "$ACCOUNT_NAME" 
 az storage container create -n "$TENANT-guest" --account-name "$ACCOUNT_NAME" 