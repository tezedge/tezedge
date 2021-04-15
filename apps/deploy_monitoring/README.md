## Monitoring

Runs and monitors the whole tezedge stack (node, debugger, explorer) inside docker contianers with informaiton sent to a slack channel.

### Prerequisites

Set up slack channel with an app that has webhooks and file:write privilages. 

### Build

```cargo build --bin deploy-monitoring --release```

### Runing

We need to set the *TEZEDGE_IMAGE_TAG* environment var to the desired version of the tezedge docker image.
Note that the debugger container needs root privilages. 

```
sudo TEZOS_NETWORK=delphinet HOSTNAME=$(hostname) ./target/release/deploy-monitoring \
--compose-file-path apps/monitoring/docker-compose.deploy.latest.yml
--image-monitor-interval 60 \ 
--resource-monitor-interval 15 \
--slack-url https://hooks.slack.com/services/XXXXXXXXX/XXXXXXXXXXX/XXXXXXXXXXXXXXXXXXXXXXXX \
--slack-token "Bearer xoxb-xxxxxxxxxxxx-xxxxxxxxxxxxx-xxxxxxxxxxxxxxxxxxxxxxxx" \
--slack-channel-name monitoring
```

If you wish to run the monitoring in the backgournd use `nohup` and redirect the output to a log file.

```
nohup <the command above> > monitoring.log &
```