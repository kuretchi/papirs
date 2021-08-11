#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum State {
    Undoing,
    Redoing,
}

#[derive(Debug)]
pub(super) struct History<C> {
    undo_stack: Vec<C>,
    redo_stack: Vec<C>,
    state: Option<State>,
}

impl<C> Default for History<C> {
    fn default() -> Self {
        Self {
            undo_stack: vec![],
            redo_stack: vec![],
            state: None,
        }
    }
}

impl<C> History<C> {
    pub fn push(&mut self, com: C) {
        match self.state {
            None => {
                self.undo_stack.push(com);
                self.redo_stack.clear();
            }
            Some(State::Undoing) => {
                self.redo_stack.push(com);
                self.state = None;
            }
            Some(State::Redoing) => {
                self.undo_stack.push(com);
                self.state = None;
            }
        }
    }

    pub fn start_undo(&mut self) -> Option<C> {
        assert!(self.state.is_none(), "previous operation not finished");
        let com = self.undo_stack.pop();
        if com.is_some() {
            self.state = Some(State::Undoing);
        }
        com
    }

    pub fn start_redo(&mut self) -> Option<C> {
        assert!(self.state.is_none(), "previous operation not finished");
        let com = self.redo_stack.pop();
        if com.is_some() {
            self.state = Some(State::Redoing);
        }
        com
    }
}
