## Watchdog

Runs and monitors the whole tezedge stack (node, debugger, explorer) inside docker contianers with informaiton sent to a slack channel.

### Prerequisites

Set up slack channel with an app that has webhooks and file:write privilages. 

### Build

```cargo build --bin watchdog --release```

### Runing

We need to set the *TEZEDGE_IMAGE_TAG* environment var to the desired version of the tezedge docker image.
Note that the debugger container needs root privilages. 

```
sudo TEZEDGE_IMAGE_TAG=latest ./target/release/watchdog \
--monitor-interval 60 \ 
--info-interval 21600 
--slack-url https://hooks.slack.com/services/XXXXXXXXX/XXXXXXXXXXX/XXXXXXXXXXXXXXXXXXXXXXXX 
--slack-token "Bearer xoxb-xxxxxxxxxxxx-xxxxxxxxxxxxx-xxxxxxxxxxxxxxxxxxxxxxxx" \
--image-tag v0.9.1 
--slack-channel-name monitoring
```

If you wish to run the watchdog in the backgournd use `nohup` and redirect the output to a log file.

```
nohup <the command above> > watchdog.log &
```