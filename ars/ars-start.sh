#!/bin/bash

while getopts i:p:a:r: flag
do
    case "${flag}" in
        i) uuid="${OPTARG}";;
        p) port="${OPTARG}";;
        a) a2s="${OPTARG}";;
        r) rcon="${OPTARG}";;
    esac
done

docker run --rm \
--network=arsa_network \
--hostname ${uuid} \
--mount type=volume,source=arsa-profiles,target=/ars/profiles \
--mount type=volume,source=arsa-servers,target=/ars/servers,readonly \
-p ${port}:${port} \
-p ${a2s}:${a2s} \
-p ${rcon}:${rcon} \
--name ${uuid} \
ars \
-config /ars/servers/${uuid}/config.json -profile /ars/profiles/${uuid} -maxFPS 60

# ./ars-start.sh -i 3c34c512-8b27-43a6-abb5-4bd5e077fcc7 -p 2001 -a 17777 -r 19999