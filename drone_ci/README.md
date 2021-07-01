# Drone - Continuous Integration platform

Our CI of choice was [drone](https://www.drone.io/). 

## Prerequisites

- Be able to ssh into the desired target hosts with public key autentification with the **SAME** username
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
    
    Before we run the playbooks, we need to set a few variables that are used in the configuration. In the vars directory, you need to 
    edit the [variables.yml](vars/variables.yml)

    There are 6 variables you need to set before continueing

    ```
    drone_server: The ip/hostname of the machine you wish to set as a server

    # User with the ssh connects to the target machines
    target_hosts_user: The username with sudo permissions on the target hosts

    # The github username you wish to add as an admin for the drone CI
    admin_user: The github username of the desired administrator

    # Variables for drone server configuration
    github_client_id: The client id of the OAuth app
    github_client_secret: The client secret of the OAuth app 
    rpc_secret: The genreated shared secret for RPC communication
    ```

    The next file you should edit, is the [hosts](inventory/hosts) file. This file is in ini format an contains all the hosts you wish to connect to.

    Example
    ```
    [drone_server]
    65.21.165.82

    [drone_runners]
    65.21.165.82
    65.21.165.83

    [real_time_runners]
    65.21.165.84

    ```

    You can add as many *drone_runners* or *real_time_runners* as you wish. Please be aware of the *real_time_runners* hardware prerequisites talked about erlier in this readme.

6. Run the ansible playbooks

    **Please keep in mind that you have to run these playbooks in this exact orded**

    Once all the variables and hosts are set you can proceed to execute the ansible playbooks.

    The first one we run is [docker_install](playbooks/docker_install.yml). This playbook prepare the hosts for the docker containers that drone runs in. It installs all the prerequisite packages for docker and docker itself.

    ```
    $ cd drone_ci
    $ ansible-playbook ./playbooks/docker_install.yml --user dev --ask-become-pass -i ./inventory/hosts
    ```

    As the next step, we run the [prepare_rt_node](playbooks/prepare_rt_node.yml). This playbook downloads and installs the required kernel for the real time environment. It ends in the reboot of the target host. Ansible will wait for the host to come online again. Again, please be aware of the *real_time_runners* hardware prerequisites talked about erlier in this readme.

    ```
    $ ansible-playbook ./playbooks/prepare_rt_node.yml --user dev --ask-become-pass -i ./inventory/hosts
    ```

    The next one to run is the [drone_server](playbooks/drone_server.yml). With this we set up and start the drone server.

    ```
    $ ansible-playbook ./playbooks/drone_server.yml --user dev --ask-become-pass -i ./inventory/hosts
    ```

    As the final touch, we setup and run all the drone runners.

    ```
    $ ansible-playbook ./playbooks/drone_runner.yml --user dev --ask-become-pass -i ./inventory/hosts
    $ ansible-playbook ./playbooks/real_time_runners.yml --user dev --ask-become-pass -i ./inventory/hosts
    ```

7. Enable and setup the tezedge repository 

    <!-- TODO: Screen capture -->

