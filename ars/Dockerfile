# steamcmd/steamcmd:latest is based on ubuntu
FROM steamcmd/steamcmd:latest

# set 'root' as current user
USER root

# install Arma Reforger Server with steamcmd to /ars
RUN steamcmd +force_install_dir /ars +login anonymous +app_update 1874900 validate +quit

# change owner of /ars to ubuntu user and group to allow executing it
RUN chown -R ubuntu:ubuntu /ars

# create group 'arsa'(gid 1337) and add user 'ubuntu' to that group to allow accessing config and profile
RUN groupadd --gid 1337 arsa && usermod -a -G arsa ubuntu

# switch to unprivileged user 'ubuntu'
USER ubuntu

# set working directory
WORKDIR "/ars"

EXPOSE 2001/tcp
EXPOSE 17777/tcp
EXPOSE 19999/tcp

ENTRYPOINT ["./ArmaReforgerServer"]