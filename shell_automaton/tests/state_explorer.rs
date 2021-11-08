#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::mem;

pub trait GenState: Clone + PartialEq + Eq + Hash + Debug {
    /// Predicate that allows limiting the size of state graph.
    fn within_bounds(&self) -> bool;
}

struct GenStateData<S> {
    /// State ID
    id: usize,
    /// Instance of specific state
    state: S,
    /// Index of the next untested action.
    next_action: usize,
    /// Known transitions out of this state, pairs of action index and state index
    known_exits: Vec<(usize, usize)>,
}

impl<S> GenStateData<S> {
    fn new(id: usize, state: S) -> Self {
        Self {
            id,
            state,
            next_action: 0,
            known_exits: Vec::new(),
        }
    }
}

struct ExplorerData<S> {
    real_state: S,
    next_action_index: usize,
}

impl<S> ExplorerData<S> {
    fn new(real_state: S) -> Self {
        Self {
            real_state,
            next_action_index: 0,
        }
    }

    fn next_action_index(&mut self) -> usize {
        let next = self.next_action_index + 1;
        mem::replace(&mut self.next_action_index, next)
    }
}

pub struct Transitions {
    pub transitions: Vec<(usize, usize)>,
}

impl Transitions {
    fn new() -> Self {
        Self {
            transitions: Vec::new(),
        }
    }

    fn add_transition(&mut self, action_index: usize, state_index: usize) {
        self.transitions.push((action_index, state_index))
    }
}

pub struct Graph<A, G> {
    pub actions: Vec<A>,
    pub states_transitions: Vec<(G, Transitions)>,
}

pub struct StateExplorer<S, A, G> {
    actions: Vec<A>,
    states: HashMap<G, usize>,
    explorer_data: Vec<ExplorerData<S>>,
    transitions: Vec<Transitions>,
    reducer: fn(&S, &A) -> Option<S>,
    undone: HashSet<G>,
}

impl<S, A, G> StateExplorer<S, A, G>
where
    S: Clone + Into<G>,
    G: GenState,
{
    pub fn new(initial: S, actions: Vec<A>, reducer: fn(&S, &A) -> Option<S>) -> Self {
        let states = HashMap::new();
        let explorer_data = Vec::new();
        let graph_data = Vec::new();
        let undone = HashSet::new();
        let mut myself = Self {
            actions,
            states,
            explorer_data,
            transitions: graph_data,
            reducer,
            undone,
        };
        let gen_state = initial.clone().into();
        myself.add_state(0, gen_state, initial);
        myself
    }

    fn add_state(&mut self, state_index: usize, gen_state: G, state: S) {
        assert_eq!(state_index, self.states.len());
        assert_eq!(state_index, self.explorer_data.len());
        assert_eq!(state_index, self.transitions.len());

        self.undone.insert(gen_state.clone());
        self.states.insert(gen_state, state_index);

        self.explorer_data.push(ExplorerData::new(state));
        self.transitions.push(Transitions::new());
    }

    pub fn explore(mut self) -> Graph<A, G> {
        let mut iteration = 0;
        while let Some(mut gen_state) = self.undone.iter().next().cloned() {
            loop {
                if iteration % 10000 == 0 {
                    print!(
                        "iteration: {}, states: {}, undone states: {}                \r",
                        iteration,
                        self.states.len(),
                        self.undone.len()
                    )
                }
                iteration += 1;
                let state_index = *self.states.get(&gen_state).unwrap();

                let gen_state_data = &mut self.explorer_data[state_index];
                let action_index = gen_state_data.next_action_index();
                if action_index >= self.actions.len() {
                    self.undone.remove(&gen_state);
                    break;
                }

                let action = &self.actions[action_index];
                if let Some(new_state) = (self.reducer)(&gen_state_data.real_state, action) {
                    let new_gen_state = new_state.clone().into();
                    if !new_gen_state.within_bounds() {
                        eprintln!("the state {:#?} is out of bounds", new_gen_state);
                        continue;
                    }
                    let (new_state_id, add_state) = self
                        .states
                        .get(&new_gen_state)
                        .map(|index| (*index, false))
                        .unwrap_or_else(|| (self.states.len(), true));
                    if add_state {
                        self.add_state(new_state_id, new_gen_state.clone(), new_state);
                    } else if new_state_id == state_index {
                        continue;
                    }
                    self.transitions[state_index].add_transition(action_index, new_state_id);
                    gen_state = new_gen_state;
                }
            }
        }
        print!(
            "iterations: {}, states: {}                                                                ",
            iteration,
            self.states.len(),
        );

        let mut states = self.states.into_iter().collect::<Vec<_>>();
        states.sort_by_key(|(_, k)| *k);
        let states_transitions = states
            .into_iter()
            .enumerate()
            .map(|(i, (s, i2))| {
                assert_eq!(i, i2);
                s
            })
            .zip(self.transitions.into_iter())
            .collect::<Vec<_>>();
        Graph {
            actions: self.actions,
            states_transitions,
        }
    }
}
