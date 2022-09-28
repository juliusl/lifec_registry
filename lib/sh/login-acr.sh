#!/bin/sh

az acr login \ 
    --expose-token \ 
    --name $REGISTRY_NAME \ 
    --output tsv \ 
    --query 'accessToken' > access_token
