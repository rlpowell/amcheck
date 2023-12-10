#!/bin/bash

# Error trapping from https://gist.github.com/oldratlee/902ad9a398affca37bfcfab64612e7d1
__error_trapper() {
  local parent_lineno="$1"
  local code="$2"
  local commands="$3"
  echo "error exit status $code, at file $0 on or near line $parent_lineno: $commands"
}
trap '__error_trapper "${LINENO}/${BASH_LINENO}" "$?" "$BASH_COMMAND"' ERR

set -euE -o pipefail

podman build -t amcheck --build-arg USERNAME=$(id -un) .

if [[ ! -d ~/.local/rust_local_amcheck ]]
then
  mkdir -p ~/.local/rust_local_amcheck
  chcon -R -t container_file_t ~/.local/rust_local_amcheck
fi

podman kill amcheck_dev || true
podman rm amcheck_dev || true

# See https://github.com/xd009642/tarpaulin/issues/1087 for the seccomp thing
# podman run --detach --name amcheck --pod amcheck_pod --security-opt seccomp=~/src/lunarvim_rust/seccomp.json -w /root/src/rust-learning/amcheck \
podman run --userns=keep-id --name amcheck_dev --security-opt seccomp=~/src/lunarvim_rust/seccomp.json -w /home/$(id -un) \
  -v ~/src:/home/$(id -un)/src -v ~/.local/rust_local_amcheck:/home/$(id -un)/.cargo \
  -it amcheck bash
