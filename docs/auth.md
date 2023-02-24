# Auth

This document has information on configuring authentication w/ the mirror.

## Using file as an access provider

Create a file at `/opt/acr/bin/.world/azurecr.io/token_cache` with the following content:

```json
{
    "refresh_token": "<refresh-token>",
    "claims": {
        "exp": 1686613092
    }
}
```

## Using username/password for registry

You can login w/ registry credentials by calling the '/login' api w/ the mirror.

Example curl request,

```sh

read -r -d '' LOGIN <<- EOM
    {
        "host": "host.registry.io",
        "username": "username",
        "password": "password"
    }
EOM

curl -X PUT 'localhost:8578/login' \ 
        -d $LOGIN
```

