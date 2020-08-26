#!/usr/bin/env python3

import binascii
import sys
import base58check

data = sys.argv[1]
key = sys.argv[2]

key = key.replace('unencrypted:', "")
header = binascii.unhexlify(data)
seed = base58check.b58decode(str.encode(key))[4:36]

from pyblake2 import blake2b
h = blake2b(digest_size=32)
h.update(b'\x01' + b'\x7a\x06\xa7\x70' + header)
digest = h.digest()

import ed25519
sk = ed25519.SigningKey(seed)
sig = sk.sign(digest)
print(sig.hex())