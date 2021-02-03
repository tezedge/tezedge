#!/usr/bin/env bash

FILES=(`find . | grep Cargo.toml`)

if [ -z "$1" ]; then
  echo "No version specified! e.g.: ./versions.sh 0.0.2"
  exit 1
fi
VERSION=$1
version_prefix="version = "

echo "Settting version to: '$VERSION'"
for file in "${FILES[@]}"
do
  old_version=`cat $file | grep ^"$version_prefix"`
  if [ -z "$old_version" ]; then
    continue
  fi

  new_version="version = \"$VERSION\""

  version_before="${old_version/#$version_prefix}"
  # replace version
  sed -i "s/$old_version/$new_version/g" $file
  version_after=`cat $file | grep ^version |  tr --delete "$version_prefix"`

  echo "$file $version_before -> $version_after"
done