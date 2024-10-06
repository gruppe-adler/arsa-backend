# steamcmd/steamcmd:latest is based on ubuntu
FROM steamcmd/steamcmd:latest

# set 'root' as current user
USER root

# install Arma Reforger Server with steamcmd to default location
RUN steamcmd +login anonymous +app_update 1874900 validate +quit

# create group 'arsa'(gid 1337) and add user 'ubuntu' to that group
RUN groupadd --gid 1337 arsa && usermod -a -G arsa ubuntu

# switch to unprivileged user 'ubuntu'
USER ubuntu

# set working directory
WORKDIR "/root/.local/share/Steam/steamapps/common/Arma Reforger Server/"

EXPOSE 2001/tcp
EXPOSE 17777/tcp
EXPOSE 19999/tcp

ENTRYPOINT ["./ArmaReforgerServer"]