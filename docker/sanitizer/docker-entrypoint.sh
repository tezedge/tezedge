#!/bin/bash -e
REPO_ROOT=/home/dev/repo
USER="dev"
GROUP="dev"

USERID="$(stat -c '%u' "$REPO_ROOT")"
GROUPID="$(stat -c '%g' "$REPO_ROOT")"


groupadd -g $GROUPID $GROUP || :

useradd -o --uid $USERID --gid $GROUPID $USER
cd /home/$USER/repo
exec /usr/sbin/gosu "$USER" /bin/bash
