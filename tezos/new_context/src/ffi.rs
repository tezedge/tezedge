use crate::{ContextKey, ContextValue, ProtocolContextApi, TezedgeContext};

// Initialization

//fn tezedge_initialize_inmem_context(rt, unit: OCamlRef<()>) {
//    let _context = storage::initializer::initialize_tezedge_context(
//        &storage::initializer::ContextKvStoreConfiguration::InMem, // NOTE: this is a config value, not a store
//    )
//    .expect("Failed to initialize merkle storage");
//    OCaml::unit()
//}

// Shell Context API

// TODO
// val exists : index -> Context_hash.t -> bool Lwt.t

// TODO
// val checkout : index -> Context_hash.t -> context option Lwt.t

// TODO
// val commit : time:Time.Protocol.t -> ?message:string -> context -> Context_hash.t Lwt.t

// TODO
// val hash : time:Time.Protocol.t -> ?message:string -> t -> Context_hash.t

// Protocol Context API

// val mem : context -> key -> bool Lwt.t
pub fn mem(context: &TezedgeContext, key: &ContextKey) -> bool {
    // TODO: remove error case or handle somehow
    context.mem(key).unwrap()
}

// val dir_mem : context -> key -> bool Lwt.t
pub fn dir_mem(context: &TezedgeContext, key: &ContextKey) -> bool {
    // TODO: remove error case or handle somehow
    context.dirmem(key).unwrap()
}

// val get : context -> key -> value option Lwt.t
pub fn get(context: &TezedgeContext, key: &ContextKey) -> Option<ContextValue> {
    // TODO: remove error case or handle somehow
    context.get(key).ok()
}

// val set : context -> key -> value -> t Lwt.t
pub fn set(context: &TezedgeContext, key: &ContextKey, value: ContextValue) -> TezedgeContext {
    // TODO: remove error case or handle somehow
    context.set(key, value).unwrap()
}

// val remove_rec : context -> key -> t Lwt.t
pub fn remove_rec(context: &TezedgeContext, key: &ContextKey) -> TezedgeContext {
    // TODO: remove error case or handle somehow
    context.delete(key).unwrap()
}

// (** [copy] returns None if the [from] key is not bound *)
// val copy : context -> from:key -> to_:key -> context option Lwt.t
pub fn copy(
    context: &TezedgeContext,
    from_key: &ContextKey,
    to_key: &ContextKey,
) -> Option<TezedgeContext> {
    // TODO: remove error case or handle somehow
    context.copy(from_key, to_key).unwrap()
}

// TODO: fold
/*
type key_or_dir = [`Key of key | `Dir of key]

(** [fold] iterates over elements under a path (not recursive). Iteration order
    is nondeterministic. *)
val fold :
  context -> key -> init:'a -> f:(key_or_dir -> 'a -> 'a Lwt.t) -> 'a Lwt.t
*/
