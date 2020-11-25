# Automatic deployment using systemd

**DISCLAIMER: this version of the automatic deployment allways cleans up ALL of the unused docker cointainers and volumes**


Follow this guide to enable automatic image updates utilising systemd jobs

## Preparation
You can find boilerplate files in the *deploy* directory. Modify the *deploy_tezedge.service* file to your specific needs.

*Example*

```
[Unit]
Description=Tezedge "stack"(node + debugger + explorer) deployment

[Service]
Type=oneshot
# give exec start the absolute path to the tezedge checking script 
ExecStart=/bin/bash /home/tezedge_user/tezedge/deploy/deploy_tezedge_stack.sh /home/tezedge_user/tezedge v0.7.2
```

## 1. Copy the systemd files to /etc/systemd/system

*You will need root privilages to do this*

```
cd deploy
sudo cp deploy_tezedge.timer /etc/systemd/system
sudo cp deploy_tezedge.service /etc/systemd/system
```

## 2. Reload the systemd daemon

```
sudo systemctl daemon-reload
```

## 3. start the service

```
sudo systemctl start deploy_tezedge.timer
```

## After start

You can verify the service timer runnin by running the command:

```
sudo systemctl start deploy_tezedge.timer
```

You can also verify the logs of the deploy script
```
sudo journalctl -e -u deploy_tezedge.service
```
