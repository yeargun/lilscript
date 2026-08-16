use std::hash::BuildHasher;

/// Fast compiler-internal hashing with reproducible iteration order.
///
/// Generated code and optimization choices can observe table iteration order,
/// so per-process random seeds are inappropriate here. Fixed-seed AHash keeps
/// its fast lookup behavior without letting process or thread scheduling alter
/// compiler output.
#[derive(Clone)]
pub(crate) struct StableBuildHasher(ahash::RandomState);

impl Default for StableBuildHasher {
    fn default() -> Self {
        Self(ahash::RandomState::with_seeds(
            0x6c69_6c73_6372_6970,
            0x742d_7374_6162_6c65,
            0x2d68_6173_682d_7631,
            0x9e37_79b9_7f4a_7c15,
        ))
    }
}

impl BuildHasher for StableBuildHasher {
    type Hasher = ahash::AHasher;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        self.0.build_hasher()
    }

    #[inline]
    fn hash_one<T: std::hash::Hash>(&self, value: T) -> u64 {
        self.0.hash_one(value)
    }
}

pub(crate) type StableHashMap<K, V> = std::collections::HashMap<K, V, StableBuildHasher>;
pub(crate) type StableHashSet<T> = std::collections::HashSet<T, StableBuildHasher>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iteration_order_is_reproducible() {
        let build = || {
            let mut map = StableHashMap::default();
            for value in [17, 3, 91, 8, 44, 2] {
                map.insert(value, value * 2);
            }
            map.into_iter().collect::<Vec<_>>()
        };
        assert_eq!(build(), build());
    }
}
