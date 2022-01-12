use crate::state::GlobalState;

pub trait ImpureAction<S> {
    fn dispatch_impure(&self, state: &mut GlobalState, service: &mut S) {
        self.reducer(state);
        self.effects(state, service);
    }

    fn reducer(&self, state: &mut GlobalState);
    fn effects(&self, state: &mut GlobalState, service: &mut S);
}

pub trait PureAction {
    fn dispatch_pure(&self, state: &mut GlobalState) {
        self.reducer(state);
    }

    fn reducer(&self, state: &mut GlobalState);
}
