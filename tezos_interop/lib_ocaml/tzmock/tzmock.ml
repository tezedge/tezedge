let twice = fun x -> 2 * x
let echo = fun x: string -> x

let _ = Callback.register "twice" twice
let _ = Callback.register "echo" echo