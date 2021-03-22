#! /bin/bash

TOGGLE_COLOR='\033[0m'
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'

# Plugins to install
# Add your plugin name here!
PLUGINS=(
    panda-gdb
    panda-il-trace
)

ROOT="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"

# Installer
install_plugin () {

    ARCH="$2"
    TARGET_DIR="$PANDA_PATH/$ARCH-softmmu/panda/plugins/"
    PLUGIN_NAME=${1//-/_}
    PLUGIN_SRC_SO="lib$PLUGIN_NAME.so"
    PLUGIN_DST_SO="$PLUGIN_NAME.so"

    pushd $ROOT/$1
    echo -e "\n${YELLOW}Bulding $1 for $2 ${TOGGLE_COLOR}"
    set -x
    cargo build --release --no-default-features --features=$ARCH
    cp ../target/release/$PLUGIN_SRC_SO $TARGET_DIR/$PLUGIN_DST_SO
    set +x
    echo -e "\n${GREEN}Installed $1 for $2 at $(realpath $TARGET_DIR) ${TOGGLE_COLOR}"
    popd
}

# Plugin-specific ------------------------------------------------------------------------------------------------------

$ROOT/panda-il-trace/setup.sh

# Plugin-agnostic ------------------------------------------------------------------------------------------------------

set -e

# Driver
for i in "${PLUGINS[@]}"
do
    install_plugin $i "x86_64"
    install_plugin $i "i386"
    install_plugin $i "mips"
    install_plugin $i "mipsel"

    # TODO: enable if support is a priority
    #install_plugin $i "ppc"

    # IL currently used by plug doesn't support ARM
    if [[ "$i" != "panda-il-trace" ]]
    then
        install_plugin $i "arm"
    else
        echo -e "\n${RED}$i does not support arm ${TOGGLE_COLOR}\n"
    fi
done
