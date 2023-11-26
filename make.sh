#!/bin/bash

podman build -t amcheck . 

podman run --rm -it -v ~/src/amcheck:/src -w /src amcheck ./make_internal.sh
