# Release checklist

Follow this checklist when creating a new release for the tezedge node.

- [ ] Create a PR with the name "Release vX.Y.Z" from develop to master.
- [ ] Copy this checklist to the description of the PR so it's easier to track.

## Prepare develop for release
- [ ] Create a PR to develop with the following changes
### Docker composes
- [ ] Update the version of the tezedge node and the node monitoring in all the docker composes included in the repository to the released version

#### Composes to update:

- [ ] [docker-compose.debug.yml](docker-compose.debug.yml)
- [ ] [docker-compose.storage.irmin.yml](docker-compose.storage.irmin.yml)
- [ ] [docker-compose.storage.memory.yml](docker-compose.storage.memory.yml)
- [ ] [docker-compose.yml](docker-compose.yml)
- [ ] [apps/node_monitoring/docker-compose.deploy.mainnet.latest-release.yml](apps/node_monitoring/docker-compose.deploy.mainnet.latest-release.yml)
- [ ] [apps/node_monitoring/docker-compose.deploy.mainnet.latest.yml](apps/node_monitoring/docker-compose.deploy.mainnet.latest.yml)
- [ ] [apps/node_monitoring/docker-compose.deploy.mainnet.snapshot.latest.yml](apps/node_monitoring/docker-compose.deploy.mainnet.snapshot.latest.yml)
- [ ] [apps/node_monitoring/docker-compose.tezedge_and_explorer.yml](apps/node_monitoring/docker-compose.tezedge_and_explorer.yml)

### CHANGELOG

- [ ] Verify that we have all the changes noted in [CHANGELOG](CHANGELOG.md).
- [ ] Change the Unrelease section of the changelog to the released version (e.g.: 1.9.1 - 2021-11-04).
- [ ] Create a new Unreleased section with all the sub-section containing "Nothing".
- [ ] Modify the links in the end of the [CHANGELOG](CHANGELOG.md) accordingly.

### Package versions

- [ ] Verify that the versions of the cargo packages are all corresponding to the new release.
- [ ] If you need to change the versions, use the [versions.sh](versions.sh) script in the root of the repository (e.g: ./versions.sh 1.9.0). Skip this step if the version is already set.

### Tutorials and demos

- [ ] Verify that the tutorials and demos are up to date with the changes included in this release.

#### Tutorials to check:

- [ ] [README.md](README.md)
- [ ] [baking/010-granadanet/README.md](baking/010-granadanet/README.md)
- [ ] [baking/mainnet/README.md](baking/mainnet/README.md)

### Mergin to develop

- [ ] Merge the above changes included in the PR to develop

## Merging to master

- [ ] Once all the CI tests pass, merge the created PR

## Creating a new release on github

- [ ] Go to https://github.com/tezedge/tezedge/releases and select `Draft a new release`.
- [ ] Put the version to be released into the `Release title` field (e.g.: v1.9.0). (This will create a new tag on publish if the tag does not exists).
- [ ] Copy the corresponding section from the CHANGELOG (the section you created from Unreleased) to the release's description.
- [ ] Click on `Publish release`.

## Prepare develop for the next version

- [ ] Use the version.sh script to increment (usually) the minor version of the packages (e.g.: the version 1.9.0 becomes 1.10.0 -> ./versions.sh 1.10.0).
- [ ] Name this commit "Version for develop" and push it to develop.
