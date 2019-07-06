use std::io::{self, Read, Write};
use std::mem;

use bytehash::{Blake2b, ByteHash};
use cryogen::{Content, Handle, Persist, Store};

#[derive(Clone)]
enum ListInner<T, H: ByteHash> {
    Empty,
    List(T, Handle<ListInner<T, H>, H>),
}

#[derive(Clone)]
struct List<T, H: ByteHash> {
    store: Store<H>,
    inner: ListInner<T, H>,
}

impl<T: Content<H> + Clone, H: ByteHash> ListInner<T, H> {
    fn push(&mut self, t: T) {
        let replaced = mem::replace(self, ListInner::Empty);
        *self = ListInner::List(t, Handle::new(replaced));
    }

    fn pop(&mut self, store: &Store<H>) -> io::Result<Option<T>> {
        let old = mem::replace(self, ListInner::Empty);
        match old {
            ListInner::Empty => Ok(None),
            ListInner::List(t, next) => {
                *self = next.get(store)?.clone();
                Ok(Some(t))
            }
        }
    }

    fn first(&self) -> Option<&T> {
        match self {
            ListInner::Empty => None,
            ListInner::List(ref t, _) => Some(t),
        }
    }

    fn first_mut(&mut self) -> Option<&mut T> {
        match self {
            ListInner::Empty => None,
            ListInner::List(ref mut t, _) => Some(t),
        }
    }
}

impl<T: Content<H> + Clone, H: ByteHash> List<T, H> {
    fn new(store: &Store<H>) -> Self {
        List {
            store: store.clone(),
            inner: ListInner::Empty,
        }
    }

    fn push(&mut self, t: T) {
        self.inner.push(t)
    }

    fn pop(&mut self) -> io::Result<Option<T>> {
        self.inner.pop(&self.store)
    }

    fn first(&mut self) -> Option<&T> {
        self.inner.first()
    }

    fn first_mut(&mut self) -> Option<&mut T> {
        self.inner.first_mut()
    }
}

impl<T: Content<H>, H: ByteHash> Persist<H> for List<T, H> {
    type Inner = ListInner<T, H>;

    fn store(&self) -> &Store<H> {
        &self.store
    }

    fn inner_mut(&mut self) -> &mut Self::Inner {
        &mut self.inner
    }

    fn from_inner_store(inner: Self::Inner, store: &Store<H>) -> Self {
        List {
            store: store.clone(),
            inner,
        }
    }
}

impl<T: Content<H>, H: ByteHash> Content<H> for ListInner<T, H> {
    fn persist(&mut self, sink: &mut dyn Write) -> io::Result<()> {
        match self {
            ListInner::Empty => sink.write_all(&[0]),
            ListInner::List(ref mut t, ref mut next) => {
                sink.write_all(&[1])?;
                t.persist(sink)?;
                next.persist(sink)
            }
        }
    }

    fn restore(source: &mut dyn Read) -> io::Result<Self> {
        let mut byte = [0u8];
        source.read(&mut byte)?;
        match byte {
            [0] => Ok(ListInner::Empty),
            [1] => Ok(ListInner::List(
                T::restore(source)?,
                Handle::restore(source)?,
            )),
            _ => panic!("invalid data"),
        }
    }

    fn children_mut(&mut self) -> &mut [Handle<Self, H>] {
        match self {
            ListInner::Empty => &mut [],
            ListInner::List(ref mut t, ref mut next) => next,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn thread_sharing() {
        let dir = tempdir().unwrap();
        let store = Store::<Blake2b>::new(&dir.path());

        let mut list = List::new(&store);

        list.push(4);
        list.push(8);
        list.push(143);
        assert_eq!(list.first().unwrap(), &143);

        let shared = list.share();

        assert_eq!(list.first().unwrap(), &143);

        let thread = std::thread::spawn(move || {
            let mut local = shared.make_local();
            assert_eq!(local.pop().unwrap().unwrap(), 143);
            assert_eq!(local.pop().unwrap().unwrap(), 8);
        });

        thread.join().unwrap();

        assert_eq!(list.pop().unwrap().unwrap(), 143);
        assert_eq!(list.pop().unwrap().unwrap(), 8);

        *list.first_mut().unwrap() = 18;
        assert_eq!(list.first().unwrap(), &18);
    }
}
