#! /bin/bash
if [ -z "$1" ]; then
  echo "No tezos root path specified"
  exit 1
fi

TEZOS_PATH="$1/tests_python"

if [ -z "$2" ]; then
  echo "No tezedge root path specified"
  exit 1
fi
TEZEDGE_PATH="$2/tezos/python-tests"

echo "copying modified test files... $TEZOS_PATth -> $TEZEDGE_PATH"
cp $TEZOS_PATH/tools/constants.py $TEZEDGE_PATH/tools/constants.py
cp $TEZOS_PATH/tools/utils.py $TEZEDGE_PATH/tools/utils.py
cp $TEZOS_PATH/daemons/node.py $TEZEDGE_PATH/daemons/node.py
cp $TEZOS_PATH/launchers/sandbox.py $TEZEDGE_PATH/launchers/sandbox.py
cp $TEZOS_PATH/tests/conftest.py $TEZEDGE_PATH/tests/conftest.py
cp $TEZOS_PATH/tests/test_basic.py $TEZEDGE_PATH/tests/test_basic.py
cp $TEZOS_PATH/tests/test_mempool.py $TEZEDGE_PATH/tests/test_mempool.py
cp $TEZOS_PATH/tests/test_baker_endorser.py $TEZEDGE_PATH/tests/test_baker_endorser.py
cp $TEZOS_PATH/tests/test_fork.py $TEZEDGE_PATH/tests/test_fork.py
cp $TEZOS_PATH/tests/test_many_nodes.py $TEZEDGE_PATH/tests/test_many_nodes.py
cp $TEZOS_PATH/tests/test_many_bakers.py $TEZEDGE_PATH/tests/test_many_bakers.py
cp $TEZOS_PATH/tests/test_multinode.py $TEZEDGE_PATH/tests/test_multinode.py
