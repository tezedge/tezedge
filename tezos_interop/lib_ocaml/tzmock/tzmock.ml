exception Exn of string

let health_check = fun x: string -> "UP - " ^ x

let get_block_header _block_header_hash =
    "0000000201dd9fb5edc4f29e7d28f41fe56d57ad172b7686ed140ad50294488b68de29474d000000005c017cd804683625c2445a4e9564bf710c5528fd99a7d150d2a2a323bc22ff9e2710da4f6d0000001100000001000000000800000000000000029bd8c75dec93c276d2d8e8febc3aa6c9471cb2cb42236b3ab4ca5f1f2a0892f6000500000003ba671eef00d6a8bea20a4677fae51268ab6be7bd8cfc373cd6ac9e0a00064efcc404e1fb39409c5df255f7651e3d1bb5d91cb2172b687e5d56ebde58cfd92e1855aaafbf05"

let apply_block (_block_header : string) (_operations : string array array) =
    "lvl 2, fit 2, prio 5, 0 ops"

let _ = Callback.register "health_check" health_check
let _ = Callback.register "get_block_header" get_block_header
let _ = Callback.register "apply_block" apply_block