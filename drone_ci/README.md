# Drone - Continuous Integration platform

Our CI of choice was [drone](https://www.drone.io/). 

## Prerequisites

- Be able to ssh into the desired target hosts with piblic key autentification
- For the real time kernel, we restrict the hardware for a one of hetzner dedicated servers, specificly to [PX-93](https://www.hetzner.com/dedicated-rootserver/px93?country=by)

## Deploying a drone ci for tezedge repository

1. OAuth Application
    - GitHub's documentation has a [step by step guide](https://docs.github.com/en/developers/apps/building-oauth-apps/creating-an-oauth-app)

    - The most important is to set the `Homepage URL` and the `Authorization callback URL` to the URL your server will run on. In our case the `Homepage URL` is "http://ci.tezedge.com" and the `Authorization callback URL` is "http://ci.tezedge.com/login"

    - After you have created the OAuth app, you will be redirected to the configuration page.

    - Create a client secret and copy it. Be carefull! You can only see the secret once. 

    <!-- TODO: screencapture -->

2. Create a shared secret

    The shared secret is used to authenticate communication between runners and your central Drone server.
    
    - To create a shared secret, run the following command
        ```
        $ openssl rand -hex 16
        bea26a2221fd8090ea38720fc445eca6
        ```

3. Install ansible

    For comfortable automation, we have created ansible playbooks to deploy a dronec ci cluster with just a few commands

    ```
    $ sudo apt update
    $ sudo apt install software-properties-common
    $ sudo add-apt-repository --yes --update ppa:ansible/ansible
    $ sudo apt install ansible
    ```

4. Fork and clone tezedge repository. Fork is important if you want to run your own CI environment

    ```
    $ git clone https://github.com/tezedgeUser/tezedge.git
    ```

    Please replace the link with your own fork of the repository

5. Edit variables and hosts 
    
    <!-- TODO: add more descriptions -->

6. Run the commands

    <!-- TODO: ansible-playbook commands go here -->

7. Enable and setup the tezedge repository 

    <!-- TODO: Screen capture -->

