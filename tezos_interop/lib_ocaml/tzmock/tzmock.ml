exception Ffi_error of string

(* BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe *)
let genesis_hash_for_alphanet = "8fcf233671b6a04fcf679d2a381c2544ea6c1ea29ba6157776ed8424affa610d"
(* block 0 *)
let block_hash_level_0_for_alphanet = "4a1cd74b27753224400dda2308dba12577072aee3a947e5525fcf9af242db3fd"
let block_level_0_for_alphanet = "00000000008fcf233671b6a04fcf679d2a381c2544ea6c1ea29ba6157776ed8424affa610d000000005c0157b0000e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a80000000085d3c506cb24f739ca2b9146a7173c65ae8fdac981ccbd359ae27aa9539f2ed0"

let current_head = ref block_level_0_for_alphanet

let echo = fun x: string -> x

let init_storage _data_dir =
    ("8eceda2f", genesis_hash_for_alphanet, block_hash_level_0_for_alphanet)

let get_current_block_header _chain_id =
    !current_head

let get_block_header block_header_hash =
    match block_header_hash with
    | "4a1cd74b27753224400dda2308dba12577072aee3a947e5525fcf9af242db3fd" ->
        raise (Ffi_error (Printf.sprintf "no header found for hash: %s" block_header_hash))
    | "8fcf233671b6a04fcf679d2a381c2544ea6c1ea29ba6157776ed8424affa610d" ->
        block_level_0_for_alphanet
    | "3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a" ->
        ""
    | _ ->
        raise (Ffi_error (Printf.sprintf "no header found for hash: %s" block_header_hash))

let apply_block (block_header_hash: string) (block_header : string) (_operations : string array array) =
    (* reset current_head *)
    current_head := block_header;
    (* return result *)
    match block_header_hash with
        | "dd9fb5edc4f29e7d28f41fe56d57ad172b7686ed140ad50294488b68de29474d" -> "activate PsddFKi32cMJ"
        | "60ab6d8d2a6b1c7a391f00aa6c1fc887eb53797214616fd2ce1b9342ad4965a4" -> "lvl 2, fit 2, prio 5, 0 ops"
        | "a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627" -> "lvl 3, fit 5, prio 12, 1 ops"
        | _ -> raise (Ffi_error (Printf.sprintf "unknown header: %s" block_header_hash))

let _ = Callback.register_exception "ffi_error" (Ffi_error "Error")
let _ = Callback.register "echo" echo
let _ = Callback.register "init_storage" init_storage
let _ = Callback.register "get_current_block_header" get_current_block_header
let _ = Callback.register "get_block_header" get_block_header
let _ = Callback.register "apply_block" apply_block