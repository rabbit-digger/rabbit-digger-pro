use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
};

use topological_sort::TopologicalSort;

#[derive(Debug)]
pub enum RootType<K> {
    All,
    Key(Vec<K>),
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
enum Node<T> {
    Item(T),
    Root,
}

pub fn topological_sort<K, V, D, E>(
    root: RootType<K>,
    map: impl Iterator<Item = (K, V)>,
    get_deps: D,
) -> Result<Option<Vec<(K, V)>>, E>
where
    K: Hash + Ord + Eq + Clone + std::fmt::Debug,
    D: Fn(&K, &V) -> Result<Vec<K>, E>,
{
    let mut ts = TopologicalSort::<Node<K>>::new();
    let mut map = map.collect::<HashMap<_, _>>();

    for (k, v) in map.iter() {
        for d in get_deps(k, v)?.into_iter() {
            ts.add_dependency(Node::Item(k.clone()), Node::Item(d));
        }
        if let RootType::All = root {
            ts.add_dependency(Node::Root, Node::Item(k.clone()));
        }
    }
    if let RootType::Key(root) = root {
        for k in root {
            ts.add_dependency(Node::Root, Node::Item(k));
        }
    }

    let mut list = VecDeque::<K>::new();
    while let Some(k) = ts.pop() {
        if let Node::Item(k) = k {
            list.push_front(k.clone());
        }
    }

    if !ts.is_empty() {
        return Ok(None);
    }

    Ok(Some(
        list.into_iter()
            .flat_map(|k| {
                let v = map.remove(&k);
                v.map(|v| (k, v))
            })
            .collect(),
    ))
}
