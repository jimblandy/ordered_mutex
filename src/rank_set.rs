pub struct RankSet<R> {
    bitset: u64,
    _rank: std::marker::PhantomData<R>,
}

impl<R> RankSet<R>
where
    R: Into<u32> + PartialOrd,
{
    pub const fn new() -> Self {
        RankSet {
            bitset: 0,
            _rank: std::marker::PhantomData,
        }
    }

    /// Insert `elt` into this set, and return `true` if the set
    /// previously contained anything greater than or equal to `elt`.
    #[inline]
    pub fn insert(&mut self, elt: R) -> bool {
        let bit = 1_u64 << elt.into();

        // Create a bitmask that includes `bit` and all bits of higher value.
        let greater_than_or_equal = !(bit - 1);
        let result = self.bitset & greater_than_or_equal != 0;
        self.bitset |= bit;
        result
    }

    #[inline]
    pub fn remove(&mut self, elt: R) {
        let bit = 1 << elt.into();
        self.bitset &= !bit;
    }
}
