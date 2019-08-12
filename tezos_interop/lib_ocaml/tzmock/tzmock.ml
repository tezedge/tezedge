exception Exn of string

let twice = fun x -> 2 * x

let echo = fun x: string -> x

let encode_operation x =
    if x = "02f2b231cb7e4a6a264ca85e3e69ef4bec6cfed8749a5ce605f43b2ca4b4d93308000002298c03ed7d454a101eb7022bc95f7e5f41ac78f80901d84f0080bd83140000e7670f32038107a59a2b9cfefae36ea21f5aa63c00931399163d0698672e8532aa15ecc679dccd6844634099e02d224cc61f6bce236819bc855e09c222f2d4056ea608c0de9d5cd633652b29cf3e1c80c6c1e0340c"
    then "{ \"contents\":\n    [ { \"kind\": \"transaction\",\n        \"source\": \"tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx\", \"fee\": \"1272\",\n        \"counter\": \"1\", \"gas_limit\": \"10200\", \"storage_limit\": \"0\",\n        \"amount\": \"42000000\",\n        \"destination\": \"tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN\" } ],\n  \"signature\":\n    \"sighEKUX4TVUc7hHHTb9GX1VJSYbDteH5TAXTA7bnhJo81X7mC99hnroqR4QoBkivq7xRr8ZeGC5oXsoBQao6ha9RHAk3i4c\" }"
    else raise (Exn "ohnoes!")


let _ = Callback.register "twice" twice
let _ = Callback.register "echo" echo
let _ = Callback.register "encode_operation" encode_operation