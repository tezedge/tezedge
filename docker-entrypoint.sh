#!/bin/bash -e

USER=dev
GROUP=dev

USERID="$(stat -c '%u' "$DOCKER_REPO_ROOT")"
GROUPID="$(stat -c '%g' "$DOCKER_REPO_ROOT")"

groupadd -g $GROUPID $GROUP || :
groupadd -g $DOCKER_GID docker || :
echo "$USER ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/user

useradd -o --uid $USERID --gid $GROUPID -G $DOCKER_GID $USER
chown $USERID:$GROUPID /tmp/bin/

exec /usr/bin/gosu "$USER" $@
