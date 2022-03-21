import json
import os.path

from tools import paths
from typing import List


def get_parameters(folder: str, network='test') -> dict:
    """Takes a protocol suffix ('alpha', '005_PsBabyM1'...) and
    retrieve json test parameters for that protocol. Assertion failure
    if parameters can't be found."""

    params_file = (
        f'{paths.TEZOS_HOME}/src/{folder}/parameters/'
        f'{network}-parameters.json'
    )
    assert os.path.isfile(params_file), (
        f'{params_file}'
        ' cannot be found; please first run'
        ' `make` in TEZOS_HOME.'
    )
    with open(params_file) as params:
        return json.load(params)


# This is the secret key used to activate a protocol from genesis in sandbox
# mode. The corresponding public key is hard-coded in the tezos node.
GENESIS_SK = "edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6"
GENESIS_PK = "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2"


IDENTITIES = {
    'bootstrap1': {
        'identity': "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx",
        'public': "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        'secret': (
            "unencrypted:"
            "edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh"
        ),
    },
    'bootstrap2': {
        'identity': "tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN",
        'public': "edpktzNbDAUjUk697W7gYg2CRuBQjyPxbEg8dLccYYwKSKvkPvjtV9",
        'secret': (
            "unencrypted:"
            "edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo"
        ),
    },
    'bootstrap3': {
        'identity': "tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU",
        'public': "edpkuTXkJDGcFd5nh6VvMz8phXxU3Bi7h6hqgywNFi1vZTfQNnS1RV",
        'secret': (
            "unencrypted:"
            "edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ"
        ),
    },
    'bootstrap4': {
        'identity': "tz1b7tUupMgCNw2cCLpKTkSD1NZzB5TkP2sv",
        'public': "edpkuFrRoDSEbJYgxRtLx2ps82UdaYc1WwfS9sE11yhauZt5DgCHbU",
        'secret': (
            "unencrypted:"
            "edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3"
        ),
    },
    'bootstrap5': {
        'identity': "tz1ddb9NMYHZi5UzPdzTZMYQQZoMub195zgv",
        'public': "edpkv8EUUH68jmo3f7Um5PezmfGrRF24gnfLpH3sVNwJnV5bVCxL2n",
        'secret': (
            "unencrypted:"
            "edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm"
        ),
    },
    'activator': {'secret': "unencrypted:" + GENESIS_SK},
}


IDENTITIES_SHORT = {'activator': {'secret': "unencrypted:" + GENESIS_SK}}

PROTO_GENESIS = 'ProtoGenesisGenesisGenesisGenesisGenesisGenesk612im'

PROTO_DEMO_NOOPS = 'ProtoDemoNoopsDemoNoopsDemoNoopsDemoNoopsDemo6XBoYp'
PROTO_DEMO_COUNTER = 'ProtoDemoCounterDemoCounterDemoCounterDemoCou4LSpdT'

ALPHA = "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK"
ALPHA_DAEMON = "alpha"  # tezos-baker-alpha
ALPHA_FOLDER = "proto_alpha"
ALPHA_PARAMETERS = get_parameters(ALPHA_FOLDER)

HANGZHOU = "PtHangz2aRngywmSRGGvrcTyMbbdpWdpFKuS4uMWxg2RaH9i1qx"
HANGZHOU_DAEMON = "011-PtHangz2"
HANGZHOU_FOLDER = "proto_011_PtHangz2"
HANGZHOU_PARAMETERS = get_parameters(HANGZHOU_FOLDER)

ITHACA = "Psithaca2MLRFYargivpo7YvUr7wUDqyxrdhC5CQq78mRvimz6A"
ITHACA_DAEMON = "012-Psithaca"
ITHACA_FOLDER = "proto_012_Psithaca"
ITHACA_PARAMETERS = get_parameters(ITHACA_FOLDER)

TEZOS_CRT = """
Certificate:
    Data:
        Version: 3 (0x2)
        Serial Number:
            f0:c1:da:d7:8e:cf:49:44:b7:69:a8:1b:89:d4:36:a5
        Signature Algorithm: sha256WithRSAEncryption
        Issuer: CN=Easy-RSA CA
        Validity
            Not Before: Apr 14 09:21:27 2021 GMT
            Not After : Jul 18 09:21:27 2023 GMT
        Subject: CN=localhost
        Subject Public Key Info:
            Public Key Algorithm: rsaEncryption
                RSA Public-Key: (2048 bit)
                Modulus:
                    00:bc:08:18:ef:f6:fd:70:f1:88:02:e7:09:a1:54:
                    ae:68:9f:6a:58:96:84:a8:a0:54:ab:8f:6d:45:7c:
                    b6:b0:56:1d:3b:a8:fd:28:5d:3b:04:b6:4b:df:c8:
                    7a:a4:4c:49:a7:53:16:d9:df:6c:83:49:7b:fd:d2:
                    34:70:af:db:d4:66:c7:ae:e0:97:b9:82:c6:c6:b9:
                    47:af:f7:39:46:2d:a3:d3:7d:b0:6f:ab:58:d8:c3:
                    3b:22:06:c1:6c:c0:ff:0c:74:7b:e6:f8:2d:f9:47:
                    21:43:af:14:e3:b5:75:56:fc:7d:d1:98:d0:6d:07:
                    aa:6e:6c:2e:2e:74:a5:24:05:6b:2f:4a:bc:a8:e8:
                    83:2a:ae:e0:f2:12:78:9b:d2:27:02:ad:a1:af:5e:
                    7a:cb:66:40:bc:94:7f:f4:cc:ab:54:e5:d9:a2:11:
                    08:6a:33:97:aa:5c:46:5d:ad:b5:ae:ca:a6:21:74:
                    89:29:01:e6:a1:df:f1:a8:58:be:10:63:67:a9:9f:
                    fa:8a:f9:dd:a5:2e:bf:00:eb:a6:2b:49:ff:95:d5:
                    a5:0f:f4:15:19:2f:72:7c:52:3c:53:81:56:cb:c3:
                    b3:e0:ea:eb:b9:18:72:cc:91:1e:0a:79:0f:b5:7c:
                    7a:f3:6f:cb:b0:a5:32:3e:36:41:b1:74:05:28:2f:
                    a2:d3
                Exponent: 65537 (0x10001)
        X509v3 extensions:
            X509v3 Basic Constraints:
                CA:FALSE
            X509v3 Subject Key Identifier:
                CB:AA:52:32:A6:25:9D:15:26:55:85:97:82:D6:EA:8B:AD:73:46:FD
            X509v3 Authority Key Identifier:
                keyid:E1:2F:3E:C9:77:09:2E:6D:6A:26:F1:7F:45:3B:30:3F:4E:D8:77:9B
                DirName:/CN=Easy-RSA CA
                serial:4D:04:D1:4A:10:52:34:02:D4:9B:45:C4:C1:BD:FD:F3:2C:EA:77:C9

            X509v3 Extended Key Usage:
                TLS Web Server Authentication
            X509v3 Key Usage:
                Digital Signature, Key Encipherment
            X509v3 Subject Alternative Name:
                DNS:localhost
    Signature Algorithm: sha256WithRSAEncryption
         61:b6:bb:32:4d:44:ee:bb:5f:5b:2d:49:9a:c5:92:ac:11:a0:
         e4:4f:d4:f4:ab:65:c0:65:92:96:55:00:7f:c3:06:a3:f6:48:
         ad:3d:67:cc:c3:71:15:de:bc:1a:f3:e2:d0:f6:e7:50:be:79:
         bc:5b:44:86:5c:06:a1:7e:bb:b1:00:ba:9a:b1:35:5a:ea:6f:
         2d:9d:73:6d:cd:9b:ec:c2:84:ea:30:b7:e7:03:88:b2:9a:2d:
         02:a2:88:6d:53:0e:ed:2d:2f:4b:88:2a:77:05:78:63:78:90:
         1b:1c:c9:10:f4:a0:22:87:cb:45:4c:44:f8:6d:75:3c:f7:11:
         a0:2e:4a:8c:01:5f:ee:5e:15:3b:f5:16:f0:de:0a:1a:99:69:
         1d:1b:b1:c6:07:95:ce:50:7f:5f:af:59:8a:aa:a8:fb:00:43:
         00:a2:a8:84:36:09:f0:68:95:d9:9a:1b:fe:22:e5:0d:1b:63:
         7f:aa:cf:80:1c:a7:07:81:b2:69:f8:13:a5:70:4a:ce:08:53:
         b7:7b:94:e8:bb:b6:65:28:8a:89:85:74:45:b9:65:1b:11:a9:
         8d:7f:5b:81:66:7d:da:7a:ac:28:ba:db:2f:01:1a:bf:d1:a5:
         01:2c:53:c0:8e:09:40:2f:22:36:a0:95:d9:be:96:b9:52:8c:
         38:17:55:1f
-----BEGIN CERTIFICATE-----
MIIDbjCCAlagAwIBAgIRAPDB2teOz0lEt2moG4nUNqUwDQYJKoZIhvcNAQELBQAw
FjEUMBIGA1UEAwwLRWFzeS1SU0EgQ0EwHhcNMjEwNDE0MDkyMTI3WhcNMjMwNzE4
MDkyMTI3WjAUMRIwEAYDVQQDDAlsb2NhbGhvc3QwggEiMA0GCSqGSIb3DQEBAQUA
A4IBDwAwggEKAoIBAQC8CBjv9v1w8YgC5wmhVK5on2pYloSooFSrj21FfLawVh07
qP0oXTsEtkvfyHqkTEmnUxbZ32yDSXv90jRwr9vUZseu4Je5gsbGuUev9zlGLaPT
fbBvq1jYwzsiBsFswP8MdHvm+C35RyFDrxTjtXVW/H3RmNBtB6pubC4udKUkBWsv
Sryo6IMqruDyEnib0icCraGvXnrLZkC8lH/0zKtU5dmiEQhqM5eqXEZdrbWuyqYh
dIkpAeah3/GoWL4QY2epn/qK+d2lLr8A66YrSf+V1aUP9BUZL3J8UjxTgVbLw7Pg
6uu5GHLMkR4KeQ+1fHrzb8uwpTI+NkGxdAUoL6LTAgMBAAGjgbgwgbUwCQYDVR0T
BAIwADAdBgNVHQ4EFgQUy6pSMqYlnRUmVYWXgtbqi61zRv0wUQYDVR0jBEowSIAU
4S8+yXcJLm1qJvF/RTswP07Yd5uhGqQYMBYxFDASBgNVBAMMC0Vhc3ktUlNBIENB
ghRNBNFKEFI0AtSbRcTBvf3zLOp3yTATBgNVHSUEDDAKBggrBgEFBQcDATALBgNV
HQ8EBAMCBaAwFAYDVR0RBA0wC4IJbG9jYWxob3N0MA0GCSqGSIb3DQEBCwUAA4IB
AQBhtrsyTUTuu19bLUmaxZKsEaDkT9T0q2XAZZKWVQB/wwaj9kitPWfMw3EV3rwa
8+LQ9udQvnm8W0SGXAahfruxALqasTVa6m8tnXNtzZvswoTqMLfnA4iymi0Cooht
Uw7tLS9LiCp3BXhjeJAbHMkQ9KAih8tFTET4bXU89xGgLkqMAV/uXhU79Rbw3goa
mWkdG7HGB5XOUH9fr1mKqqj7AEMAoqiENgnwaJXZmhv+IuUNG2N/qs+AHKcHgbJp
+BOlcErOCFO3e5Tou7ZlKIqJhXRFuWUbEamNf1uBZn3aeqwoutsvARq/0aUBLFPA
jglALyI2oJXZvpa5Uow4F1Uf
-----END CERTIFICATE-----
"""


TEZOS_KEY = """
-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQC8CBjv9v1w8YgC
5wmhVK5on2pYloSooFSrj21FfLawVh07qP0oXTsEtkvfyHqkTEmnUxbZ32yDSXv9
0jRwr9vUZseu4Je5gsbGuUev9zlGLaPTfbBvq1jYwzsiBsFswP8MdHvm+C35RyFD
rxTjtXVW/H3RmNBtB6pubC4udKUkBWsvSryo6IMqruDyEnib0icCraGvXnrLZkC8
lH/0zKtU5dmiEQhqM5eqXEZdrbWuyqYhdIkpAeah3/GoWL4QY2epn/qK+d2lLr8A
66YrSf+V1aUP9BUZL3J8UjxTgVbLw7Pg6uu5GHLMkR4KeQ+1fHrzb8uwpTI+NkGx
dAUoL6LTAgMBAAECggEAJA5Ncd5j1P+LvDq/Xv9U/lzrUJd0Ur2D3u3+3x8+DOxG
aMVL3iyaf5nRTNUtp0m1qe9F53tvXHF/5DklsyIVvlIDImaQ0ZLhOQQYWgbHJczk
qE5mwdMSk2ZEdY7kEk2j2qiPhce7URyxpT/yeoO2P3rlSYbLGM0qgkgeRuw5KoHH
Vq7w/NmgFuhDeqmYi7C+51CCD2x56la08VW/xRNATzniVZ97CvpwWhjdKock2+bp
FB6FE77jOye0uqdEcsPl6cJfJaL6vQRl9h8DFn2GmAnRP5mcqx03HaehPrwO86mC
xlrPS0CZeti1Y3VVZCFEKmGy8bYWLBSoXAgeWEtQWQKBgQDfeoT3STBdHdBVpkAW
9wXhzmu1ZyXNcDM9j64fuZ7oWitdoQM6znfws2c1jNcXIvxuTRWm7OJGT1ak/gck
2+4g6soQbg4vtGzxpOSmzp4MobDip/cjwWDoYC0OQZHZT/PImaTP4AAXjQAL8S1H
mty9DnLHOCBd7V1xVKM6U8hY3wKBgQDXZQkVNhwRlk3Sbrxgb5/J6zMLbjO4XDm6
olLT9hnUxhYZ06voaC9fP52WiswUaSOc6LBC2SR5mLxFBNjBuMekYG5z+4KJvdj8
DyX/ZHtIDSvC2Iol0uO9ppcoHUafXbHXxBLskOxsC45ggK79lpuUghc/mVCz7D0s
BGWDDg9QjQKBgQCFAkjtXVQ5t6rtrztp77BCizc0CqZHNcZpl4CNRU88/53b5h8j
+wsL6ds91guWq64OgDao2Uh7jHEHVmIuH/AFC3kkejxbTEmjMP8eAM+0uO+sl0fS
sh/ZbpSibYg/DQUNmdSsHKgxXCxw7ySB/7vtkhHiXJd3D/WTpEpaRs9xhQKBgB8x
d55FxszZOo32EXvZzoc8c5j9LapOWOHpbhtaMaV5xmuZFvVCWVHu8ZCCq0ltbIXl
wNj9f2XIs8M/D3EGpIrumDBdxSrTfqAKRZN15tCpb6P5HhCaOPcXMB7UFo0v0XiQ
4bi2yDZheg4JtM3uyLs6F8nTFzfnR3ifbmALYjZlAoGBANqsFDHh7v/xSKDnBV8k
sOL4Bzv4U42jueGl/p37bflgn64ApCBaMCO943hUt6XUgnX+9EMFY8+Qi7CdtVRK
UP3J3otOYjHnDCLK/Re6KnxyB0Xj10+jOEqceoSdZnByKtfyFmfdn/0U5F8IhbbI
V7GSZfaUrOrd1H6Tmb23vlSk
-----END PRIVATE KEY-----
"""

# Required to be install for TLS related tests to succeed
TEZOS_CERT = """
-----BEGIN CERTIFICATE-----
MIIDSzCCAjOgAwIBAgIUTQTRShBSNALUm0XEwb398yzqd8kwDQYJKoZIhvcNAQEL
BQAwFjEUMBIGA1UEAwwLRWFzeS1SU0EgQ0EwHhcNMjEwNDE0MDkyMDEyWhcNMzEw
NDEyMDkyMDEyWjAWMRQwEgYDVQQDDAtFYXN5LVJTQSBDQTCCASIwDQYJKoZIhvcN
AQEBBQADggEPADCCAQoCggEBAJmECPamqSiCzfX5ijAQQ0jAqT5W/CU2e9ceF603
rx3/1EvC+fFJqbLRpcydK6KxHKxtyrkz0PKU0Fg9DOhNpfc7vQsrI/S1qKJ1fS2S
PxgU3/NakLlIZ93LDexdSFQAzTitjP8sRtRZIbpo00yzggWhoLQR9ayiEtZ/4M7a
VZDy+1QvG0y+dvYwsD2ArLkNyrQP2Z1XhAlpPlL4MgmtW8DZ/YODgePyP9x/6KR+
oSgNg+0lQZNqC76X/E5EWXiYCKpYHMIWuaNSk1/dSv+MzdOqpKZkSiDHTDTG8aLX
7tcP2WM6WRORfl51Fsp/fYOr2emS+ePkTmN8O0R0ZiNGkT0CAwEAAaOBkDCBjTAd
BgNVHQ4EFgQU4S8+yXcJLm1qJvF/RTswP07Yd5swUQYDVR0jBEowSIAU4S8+yXcJ
Lm1qJvF/RTswP07Yd5uhGqQYMBYxFDASBgNVBAMMC0Vhc3ktUlNBIENBghRNBNFK
EFI0AtSbRcTBvf3zLOp3yTAMBgNVHRMEBTADAQH/MAsGA1UdDwQEAwIBBjANBgkq
hkiG9w0BAQsFAAOCAQEASW0O/IjCZ9mrdV4egG+QIpcOkmqm+p/WIAWPn0b0le8a
KgooeUTxvT8SCj3AZ9pcBGvajgqNt9rzpzmiueRZ+D34T/rVqsuPyEvsexKPzDgp
3IigI7iOXG4mKxAOrrKrKx/A0880f6GsR8WpDkk+MfKrkhrDeGyjLwNrEqiH1mzm
EkSTLzNuv6MaftGIQdIwyz95qwBxXoxRjyF3hfeBpLvuLZVhkw2L2sWpXTmeMY+e
kj9Z54Xdhadl1gj3AHxin5CJmXqxJ/R1JMd7IKG4kT7l5+MlbMWnaW3wM4HdUjrI
C0SRocQ//CD7E8xYVpKf1y5zD7IQGwY5qOxVK6l8kw==
-----END CERTIFICATE-----
"""

"""
Default node parameters.

A high-number of connections helps triggering the maintenance process
 more often, which speeds up some tests. A synchronisation threshold of 0
 ensures all nodes are bootstrapped when they start, which can avoid
 some spurious deadlocks (e.g. a node not broadcasting its head).
"""
OCTEZ_NODE_PARAMS = ['--connections', '100', '--synchronisation-threshold', '0']

TEZEDGE_NODE_PARAMS = [
    '--sandbox-patch-context-json-file',
    paths.TEZOS_HOME + 'sandbox-patch-context.json',
    '--bootstrap-db-path',
    'light-node',
    '--log-format',
    'simple',
    # '--ocaml-log-enabled',
    '--protocol-runner',
    paths.TEZOS_HOME + 'protocol-runner',
    '--peer-thresh-low',
    '250',
    '--peer-thresh-high',
    '500',
    '--disable-peer-graylist',
    '--compute-context-action-tree-hashes',
    '--tokio-threads=0',
    '--enable-testchain',
    '--log-level=debug',
    '--synchronization-thresh',
    '0',
    # zcash-params files used for init, if zcash-params is not correctly setup it in OS
    '--init-sapling-spend-params-file',
    paths.TEZOS_HOME + 'sapling-spend.params',
    '--init-sapling-output-params-file',
    paths.TEZOS_HOME + 'sapling-output.params',
]


class NodeParams:
    def __init__(
        self,
        octez_params: List[str],
        tezedge_params: List[str],
        more_params: List[str] = [],
    ):
        self._octez_params = octez_params
        self._tezedge_params = tezedge_params
        self._more_params = more_params

    def __add__(self, more_params: List[str]):
        return NodeParams(
            self._octez_params,
            self._tezedge_params,
            self._more_params + more_params,
        )

    def __getitem__(self, node_name: str):
        if node_name == 'tezos-node':
            return self._octez_params + self._more_params
        elif node_name == 'light-node':
            return self._tezedge_params + self._more_params
        else:
            raise Exception(
                message="NodeParams: Unknown node type: " + node_name
            )


NODE_PARAMS = NodeParams(
    octez_params=OCTEZ_NODE_PARAMS, tezedge_params=TEZEDGE_NODE_PARAMS
)

TENDERBAKE_BAKER_LOG_LEVELS = {"alpha.baker.*": "debug"}

TENDERBAKE_NODE_LOG_LEVELS = {
    "node.chain_validator": "debug",
    "node.validator.block": "debug",
    "node_prevalidator": "debug",
    "validator.block": "debug",
    "validator.chain": "debug",
}
