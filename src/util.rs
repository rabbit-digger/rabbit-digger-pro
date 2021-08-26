use std::{collections::BTreeMap, hash::Hash};

use topological_sort::TopologicalSort;

pub fn topological_sort<K, V, D, E>(
    mut map: BTreeMap<K, V>,
    get_deps: D,
) -> Result<Option<Vec<(K, V)>>, E>
where
    K: Hash + Ord + Eq + Clone,
    D: Fn(&K, &V) -> Result<Vec<K>, E>,
{
    let mut ts = TopologicalSort::<K>::new();

    for (k, v) in map.iter() {
        for d in get_deps(k, v)?.into_iter() {
            ts.add_dependency(d, k.clone());
        }
        ts.insert(k.clone());
    }

    let mut list = Vec::<K>::new();
    while let Some(k) = ts.pop() {
        list.push(k.clone());
    }

    if !ts.is_empty() {
        return Ok(None);
    }

    Ok(Some(
        list.into_iter()
            .map(|k| {
                let v = map.remove(&k);
                v.map(|v| (k, v))
            })
            .flatten()
            .collect(),
    ))
}
