// lanes/cycles.rs — the strongly connected components of a graph, found without recursion: a chain of 100,000 timelines must not overflow.
// A reader would get wrong that it knows nothing of timelines: nodes are indexes, and the caller says which edge it is asking about (261).

/// The component of each node of a directed graph given as adjacency lists.
#[derive(Debug)]
pub struct Sccs {
    comp: Vec<usize>,
}

impl Sccs {
    /// Tarjan's algorithm with an explicit stack: linear in nodes and edges.
    pub fn of(adj: &[Vec<usize>]) -> Sccs {
        let n = adj.len();
        let none = usize::MAX;
        let (mut index, mut low) = (vec![none; n], vec![0usize; n]);
        let mut comp = vec![none; n];
        let (mut on_stack, mut stack) = (vec![false; n], Vec::new());
        let (mut next_index, mut next_comp) = (0usize, 0usize);
        // A frame is a node and how far into its edges the walk has gone.
        let mut frames: Vec<(usize, usize)> = Vec::new();
        for root in 0..n {
            if index[root] != none {
                continue;
            }
            frames.push((root, 0));
            while let Some(&(v, at)) = frames.last() {
                if at == 0 && index[v] == none {
                    index[v] = next_index;
                    low[v] = next_index;
                    next_index += 1;
                    stack.push(v);
                    on_stack[v] = true;
                }
                if let Some(&w) = adj[v].get(at) {
                    if let Some(top) = frames.last_mut() {
                        top.1 += 1;
                    }
                    if index[w] == none {
                        frames.push((w, 0));
                    } else if on_stack[w] {
                        low[v] = low[v].min(index[w]);
                    }
                    continue;
                }
                frames.pop();
                if let Some(&(parent, _)) = frames.last() {
                    low[parent] = low[parent].min(low[v]);
                }
                if low[v] == index[v] {
                    while let Some(w) = stack.pop() {
                        on_stack[w] = false;
                        comp[w] = next_comp;
                        if w == v {
                            break;
                        }
                    }
                    next_comp += 1;
                }
            }
        }
        Sccs { comp }
    }

    /// Whether `a` and `b` lie on a cycle together (or are the same node).
    pub fn same_scc(&self, a: usize, b: usize) -> bool {
        a == b || (self.comp.get(a).is_some() && self.comp.get(a) == self.comp.get(b))
    }
}
