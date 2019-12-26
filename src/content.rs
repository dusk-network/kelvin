use std::io::{self, Read, Write};
use std::marker::PhantomData;

use bytehash::ByteHash;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use crate::sink::Sink;
use crate::source::Source;

/// The main trait for content-adressable types, MUST assure a 1-1 mapping between
/// values of the type and hash digests.
pub trait Content<H: ByteHash>
where
    Self: Sized + Clone + 'static,
{
    /// Write the type to a `Sink`
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()>;
    /// Restore the type from a `Source`
    fn restore(source: &mut Source<H>) -> io::Result<Self>;
}

impl<T: Content<H>, H: ByteHash> Content<H> for Option<T> {
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        match *self {
            Some(ref mut content) => {
                sink.write_all(&[1])?;
                content.persist(sink)
            }
            None => sink.write_all(&[0]),
        }
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let mut byte = [0u8];
        source.read_exact(&mut byte)?;
        match byte[0] {
            0 => Ok(None),
            1 => Ok(Some(T::restore(source)?)),
            _ => panic!("Invalid Option encoding"),
        }
    }
}

impl<T: Content<H>, H: ByteHash> Content<H> for Box<T> {
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        (**self).persist(sink)
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        Ok(Box::new(T::restore(source)?))
    }
}

impl<H: ByteHash> Content<H> for () {
    fn persist(&mut self, _: &mut Sink<H>) -> io::Result<()> {
        Ok(())
    }

    fn restore(_: &mut Source<H>) -> io::Result<Self> {
        Ok(())
    }
}

impl<X: 'static, H: ByteHash> Content<H> for PhantomData<X> {
    fn persist(&mut self, _: &mut Sink<H>) -> io::Result<()> {
        Ok(())
    }
    fn restore(_: &mut Source<H>) -> io::Result<Self> {
        Ok(::std::marker::PhantomData)
    }
}

impl<H: ByteHash> Content<H> for u8 {
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        sink.write_all(&[*self])?;
        Ok(())
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let mut byte = [0u8];
        source.read_exact(&mut byte)?;
        Ok(byte[0])
    }
}

impl<H: ByteHash> Content<H> for String {
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        let bytes = self.as_bytes();
        sink.write_u64::<BigEndian>(bytes.len() as u64)?;
        sink.write_all(&bytes)?;
        Ok(())
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let byte_len = source.read_u64::<BigEndian>()?;
        let mut take = source.take(byte_len);
        let mut string = String::new();
        take.read_to_string(&mut string)?;
        Ok(string)
    }
}

impl<H: ByteHash, T: Content<H>> Content<H> for Vec<T> {
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        sink.write_u64::<BigEndian>(self.len() as u64)?;
        for t in self.iter_mut() {
            t.persist(sink)?
        }
        Ok(())
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let len = source.read_u64::<BigEndian>()?;
        let mut vec = Vec::with_capacity(len as usize);
        for _ in 0..len {
            vec.push(T::restore(source)?)
        }
        Ok(vec)
    }
}

// numbers
macro_rules! number {
    ($t:ty : $read:ident, $write:ident) => {
        impl<H: ByteHash> Content<H> for $t {
            fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
                sink.$write::<BigEndian>(*self)
            }

            fn restore(source: &mut Source<H>) -> io::Result<Self> {
                source.$read::<BigEndian>()
            }
        }
    };
}

number!(u128: read_u128, write_u128);
number!(u64: read_u64, write_u64);
number!(u32: read_u32, write_u32);
number!(u16: read_u16, write_u16);

number!(i128: read_i128, write_i128);
number!(i64: read_i64, write_i64);
number!(i32: read_i32, write_i32);
number!(i16: read_i16, write_i16);

impl<A, B, H> Content<H> for (A, B)
where
    A: Content<H>,
    B: Content<H>,
    H: ByteHash,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        self.0.persist(sink)?;
        self.1.persist(sink)
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        Ok((A::restore(source)?, B::restore(source)?))
    }
}
