Tezos client
==============

Purpose of this module is encapsulate FFI calls (tezos_interop module), which use low-level structures - string, array of strings...

Tezos client layer uses high-level structures (shell/p2p structures and encodings) for better manipulation and reading the code.

Tezos client also supports Tezos specific configuration for different Nets (Alhanet, Zeronet, Mainnet, Babylonnet) reflecting Tezos source codes like:
 - `chain genesis`
 - `default bootstrap peers`