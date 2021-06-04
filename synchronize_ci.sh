#!/usr/bin/env bash

PATH_TO_SYNC=$1
TARGET_PATH=$2
RSA_PATH=$3

CI_HOSTS=(65.21.119.66 65.21.119.67 65.21.119.68 65.21.119.69 65.21.119.70 65.21.165.82 65.21.165.83 65.21.165.84 65.21.165.85)

# exclude pushing host (do not synch with self) 
for HOST in "${CI_HOSTS[@]}"
do
    if [[ $HOST != $DRONE_RUNNER_IP_ADDRESS ]]; then
        echo "Syncing to $HOST, rsa_path: $RSA_PATH"
        rsync -e "ssh -o StrictHostKeyChecking=no -i $RSA_PATH/id_rsa" -a --progress $PATH_TO_SYNC/ dev@$HOST:$TARGET_PATH
    fi
done
