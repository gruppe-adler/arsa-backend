docker run -d --rm \
--mount type=volume,source=arsa-profiles,target=/ars/profiles \
--mount type=volume,source=arsa-servers,target=/ars/servers,readonly \
-p 2001:2001 \
-p 17777:17777 \
-p 19999:19999 \
--name ars-1 \
ars \
-config /ars/servers/81fa520c-fcc8-4a3e-b533-0a8211687f9a/config.json -profile /ars/profiles/81fa520c-fcc8-4a3e-b533-0a8211687f9a -maxFPS 60

# BattleEye needs to be false in order to start ars. I think it's related to the battleye library