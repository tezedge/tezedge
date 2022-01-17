use crate::state::GlobalState;

pub trait ImpureAction<S> {
    fn dispatch_impure(&self, state: &GlobalState, service: &mut S) {
        self.effects(state, service);
    }

    fn effects(&self, state: &GlobalState, service: &mut S);
}

pub trait PureAction {
    fn dispatch_pure(&self, state: &GlobalState) {
        self.reducer(state);
    }

    fn reducer(&self, state: &GlobalState);
}
