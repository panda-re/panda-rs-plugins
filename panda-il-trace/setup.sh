# See: https://github.com/falconre/falcon/blob/master/Dockerfile

SUDO=''
if (( $EUID != 0 )); then
    SUDO='sudo'
fi

# Remove installed capstone version
$SUDO apt-get remove -y libcapstone3 libcapstone-dev

# Build from source
mkdir temp
cd temp/
wget -nc -q --show-progress https://github.com/aquynh/capstone/archive/4.0.2.tar.gz
tar xf 4.0.2.tar.gz
cd capstone-4.0.2
make -j
$SUDO make install
cd ../../
rm -rf temp/
