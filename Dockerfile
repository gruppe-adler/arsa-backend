FROM denoland/deno:alpine

# the group id of the group docker of your docker host
ARG DOCKER_GID="989"

# set 'root' as current user
USER root

# install docker-cli to allow starting containers on host
RUN apk add docker-cli-compose

# create and set working directory
WORKDIR /app

# copy all backend files to working directory
COPY . /app/

# create group 'arsa'(gid 1337) and add user 'deno' to that group
RUN addgroup --gid 1337 arsa && adduser deno arsa

# create folders and set group to 'arsa'(gid 1337)
RUN install -d -m 775 -g arsa /app/profiles && install -d -m 775 -g arsa /app/servers

# create group 'docker'(gid from arg) and add user 'deno' to that group
# this group is defined on the docker host; needs to be adjusted in production
RUN addgroup --gid ${DOCKER_GID} docker && adduser deno docker

# switch to unprivileged user 'deno'
USER deno

EXPOSE 3000

ENTRYPOINT ["deno", "task", "prod"]