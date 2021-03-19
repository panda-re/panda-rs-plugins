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

# Installer
install_plugin () {

    ARCH="$2"
    TARGET_DIR="$PANDA_PATH/$ARCH-softmmu/panda/plugins/"

    cd $1
    echo -e "\n${YELLOW}Bulding $1 for $2 ${TOGGLE_COLOR}"
    set -x
    cargo build --release --no-default-features --features=$ARCH
    find ../target/release -maxdepth 1 -iname "libpanda*.so" -exec cp "{}" $TARGET_DIR \;
    set +x
    echo -e "\n${GREEN}Installed $1 for $2 at $(realpath $TARGET_DIR) ${TOGGLE_COLOR}"
    cd ..
}

# Plugin-specific ------------------------------------------------------------------------------------------------------

cd panda-il-trace
bash ./setup.sh
cd ..

# Plugin-agnostic ------------------------------------------------------------------------------------------------------

set -e

# Driver
for i in "${PLUGINS[@]}"
do
    install_plugin $i "x86_64"
    # TODO: default feature error for panda-re-sys
    #install_plugin $i "i386"
    #install_plugin $i "mips"
    #install_plugin $i "mipel"
    #install_plugin $i "ppc"

    # IL currently used by plug doesn't support ARM
    if [[ "$i" != "panda-il-trace" ]]
    then
        # TODO: default feature error for panda-re-sys
        #install_plugin $i "arm"
        echo ""
    else
        echo -e "\n${RED}$i does not support arm ${TOGGLE_COLOR}"
    fi
done

# Cleanup --------------------------------------------------------------------------------------------------------------

# panda-il-trace: revert to PANDA-compatible version after we've built our shared lib plugin
sudo apt-get install -y libcapstone3 libcapstone-dev