pub struct State {
    pathfinder: crate::path_finding::PathFinder,
}

impl State {
    pub fn new() -> Self {
        State {
            // Initialize fields as necessary
            pathfinder: crate::path_finding::PathFinder::new(),
        }
    }
}
