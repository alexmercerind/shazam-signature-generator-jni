#!/bin/bash

# Print commands, exit on error
set -xe

# Go to script's directory
cd "$(dirname "$0")"

ORIG_DIR="$(pwd)"

# Create a target directory for our new source package before we build it
temp_dir="$(mktemp -d)"

function cleanup_dirs {
    rm -rf "${temp_dir}"
}

trap cleanup_dirs INT TERM

for version in bionic focal groovy hirsute; do

    rm -rf ../../target/ ../../vendor/ ../../.flatpak-builder ../flatpak/.flatpak-builder ../../repo ../../.cargo

    cp -ra ../../ "${temp_dir}/songrec-0.1.2~3${version}"

    cd "${temp_dir}/songrec-0.1.2~3${version}"

    mkdir -p .cargo
    cargo vendor vendor | sed 's/^directory = ".*"/directory = "vendor"/g' > .cargo/config

    # "dpkg-source" will destroy the ".gitignore" files from source archive anyway.
    # Prevent "cargo" to check for their presence.
    find vendor -name .cargo-checksum.json -exec sed -ri 's/"[^"]*?\.gitignore":"[^"]+?"[,\}]//g' '{}' \;

    mv packaging/ppa/debian .
    
    sed -ri "s/\) bionic/${version}) ${version}/g" debian/changelog

    debuild -S -sa

    rm -f /tmp/songrec*

    mv ../*.tar* ../../ || :
    mv ../*.dsc* ../../ || :
    mv ../*.deb* ../../ || :
    mv ../*changes* ../../ || :
    mv ../*build* ../../ || :
    mv ../*source* ../../ || :

    # Push to Launchpad

    dput ppa:marin-m/songrec "../../songrec_0.1.2~3${version}_source.changes"

    cd "${ORIG_DIR}"

    rm -rf "${temp_dir}/songrec-0.1.2~3${version}"

done

cleanup_dirs

echo 'Package successfully uploaded to Launchpad, find it in /tmp'
