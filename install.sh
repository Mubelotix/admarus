#!/bin/sh

green='\033[0;32m'
red='\033[1;31m'
normal='\033[0m'

# This script uses the Rustup script as a library to detect the architecture of the system
source_rustup_functions() {
    echo "[1/3] Downloading library"
    rust_script=$(curl --proto '=https' --tlsv1.2 -sS https://sh.rustup.rs)
    if [ $? -ne 0 ]; then
        echo "${red}Error: Failed to download library${normal}"
        return 1
    fi

    last_line=$(echo "$rust_script" | tail -n 1)
    if [ "$last_line" != 'main "$@" || exit 1' ]; then
        echo "${red}Error: An update to the Rustup script has broken this script.${normal} Please open an issue at https://github.com/Mubelotix/admarus/issues"
        return 1
    fi

    total_lines=$(echo "$rust_script" | wc -l)
    total_lines=$((total_lines - 1))
    rust_script=$(echo "$rust_script" | head -n $total_lines)
    eval "$rust_script"
}

source_rustup_functions

get_architecture
arch="$RETVAL"
filename="admarusd_${arch}"
latest_url=$(curl -sSL -w "%{url_effective}" -o /dev/null "https://github.com/Mubelotix/admarus/releases/latest")
version=$(echo "$latest_url" | sed 's:.*/::')
download_url="https://github.com/mubelotix/admarus/releases/download/$version/admarusd-$arch"

echo "[2/3] Downloading admarusd $version"
curl --fail --location --progress-bar "$download_url" -o "/tmp/$filename"
case $? in
    0)  ;;
    22) echo "${red}No available binary for your system.${normal} Please build from source: https://github.com/Mubelotix/admarus/wiki/building-from-source"; exit 1 ;;
    *)  echo "${red}Error: Failed to download admarusd binary${normal}"; exit 1 ;;
esac

echo "[3/3] Installing admarusd at /user/local/bin/admarusd"
sudo mv "/tmp/$filename" "/usr/local/bin/admarusd"
chmod +x "/usr/local/bin/admarusd"

echo "${green}Admarus $version has been installed successfully!${normal}"
