// Example on how to use extra hash annotations to do merkle proofs
use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use std::io;

use kelvin::{
    Blake2b, ByteHash, Combine, Compound, Content, ErasedAnnotation, Sink,
    Source,
};
use kelvin_hamt::{HAMTSearch, HAMT};
use std::collections::hash_map::DefaultHasher;

#[derive(Clone, Debug)]
struct HashAnnotation(u64);

impl<A> Combine<A> for HashAnnotation {
    fn combine<E>(elements: &[E]) -> Option<Self>
    where
        A: Borrow<Self> + Clone,
        E: ErasedAnnotation<A>,
    {
        let mut hasher = DefaultHasher::new();
        for element in elements {
            if let Some(annotation) = element.annotation() {
                let h: &HashAnnotation = (*annotation).borrow();
                h.0.hash(&mut hasher);
            }
        }
        Some(HashAnnotation(hasher.finish()))
    }
}

impl<H> Content<H> for HashAnnotation
where
    H: ByteHash,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        self.0.persist(sink)
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        Ok(HashAnnotation(u64::restore(source)?))
    }
}

impl<T> From<&T> for HashAnnotation
where
    T: Hash,
{
    fn from(t: &T) -> Self {
        let mut hasher = DefaultHasher::new();
        t.hash(&mut hasher);
        HashAnnotation(hasher.finish())
    }
}

#[test]
fn merkle_proof() {
    use kelvin::Proof;

    let mut hamt = HAMT::<_, _, HashAnnotation, Blake2b>::new();

    for i in 0..32 {
        hamt.insert(i, i).unwrap();
    }
    // make a proof that (0, 0) is in the hamt

    let mut proof = {
        let mut branch =
            hamt.search_mut(&mut HAMTSearch::from(&0)).unwrap().unwrap();

        Proof::new(&mut branch)
    };

    assert!(proof.valid(&mut hamt));
}
