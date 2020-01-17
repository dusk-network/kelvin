use std::cmp;
use std::fmt;
use std::io::{self, Write};

use kelvin::{ByteHash, Content, Sink, Source};
use smallvec::SmallVec;

// The maximum number of bytes to represent segments inline, before spilling to heap
//
// This is to make a compromize between wasting space in the memory representation,
// and avoiding unnecessary heap lookups.
const SEGMENT_SIZE: usize = 4;

pub trait AsNibbles {
    fn as_nibbles(&self) -> Nibbles;
}

impl<'a> AsNibbles for Nibbles<'a> {
    fn as_nibbles(&self) -> Nibbles {
        Nibbles {
            bytes: &self.bytes,
            ofs_front: self.ofs_front,
            ofs_back: self.ofs_back,
        }
    }
}

impl<'a, A> From<&'a A> for Nibbles<'a>
where
    A: AsRef<[u8]> + ?Sized,
{
    fn from(a: &'a A) -> Self {
        let bytes = a.as_ref();
        Nibbles {
            bytes,
            ofs_front: 0,
            ofs_back: bytes.len() * 2,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Nibbles<'a> {
    bytes: &'a [u8],
    ofs_front: usize,
    ofs_back: usize,
}

impl<'a> Nibbles<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Nibbles {
            bytes,
            ofs_front: 0,
            ofs_back: bytes.len() * 2,
        }
    }

    pub fn get(&self, idx: usize) -> usize {
        let byte_index = (self.ofs_front + idx) / 2;
        let byte = self.bytes[byte_index];

        // pick the right nibble from the byte
        // when front + offset is even, we pick the first
        (if (idx + self.ofs_front) % 2 == 0 {
            (byte & 0xF0) >> 4
        } else {
            byte & 0x0F
        }) as usize
    }

    pub fn pop_nibble(&mut self) -> usize {
        let nibble = self.get(0);
        self.ofs_front += 1;
        debug_assert!(self.ofs_front <= self.ofs_back);
        nibble
    }

    pub fn trim_front(&mut self, by: usize) {
        self.ofs_front += by;
        debug_assert!(self.ofs_front <= self.ofs_back)
    }

    pub fn trim_back(&mut self, by: usize) {
        self.ofs_back -= by;
        debug_assert!(self.ofs_front <= self.ofs_back)
    }

    pub fn len(&self) -> usize {
        self.ofs_back - self.ofs_front
    }

    pub fn common_prefix<A: AsNibbles>(&self, other: &A) -> Nibbles {
        let len = self.common_prefix_len(&other.as_nibbles());
        let mut nibbles = self.clone();
        nibbles.trim_back(self.len() - len);
        nibbles
    }

    fn common_prefix_len(self, other: &Self) -> usize {
        let min_len = cmp::min(self.len(), other.len());
        for i in 0..min_len {
            if self.get(i) != other.get(i) {
                return i;
            }
        }
        return min_len;
    }
}

impl<'a> fmt::Debug for Nibbles<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let len = self.len();
        write!(f, "[")?;
        if len > 0 {
            for i in 0..len - 1 {
                write!(f, "{:x}, ", self.get(i))?;
            }
            write!(f, "{:x}", self.get(len - 1))?;
        }
        write!(f, "]")
    }
}
#[derive(Clone, Default)]
pub struct NibbleBuf {
    bytes: SmallVec<[u8; SEGMENT_SIZE]>,
    ofs_front: usize,
    ofs_back: usize,
}

impl PartialEq for NibbleBuf {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for i in 0..self.len() {
            if self.get(i) != other.get(i) {
                return false;
            }
        }
        true
    }
}

impl NibbleBuf {
    pub fn new(bytes: &[u8]) -> Self {
        NibbleBuf {
            bytes: bytes.into(),
            ofs_front: 0,
            ofs_back: bytes.len() * 2,
        }
    }

    pub fn get(&self, i: usize) -> usize {
        self.as_nibbles().get(i)
    }

    pub fn len(&self) -> usize {
        self.ofs_back - self.ofs_front
    }

    pub fn trim_front(&mut self, by: usize) {
        self.ofs_front += by;
        debug_assert!(self.ofs_front <= self.ofs_back)
    }

    pub fn trim_back(&mut self, by: usize) {
        self.ofs_back -= by;
        debug_assert!(self.ofs_front <= self.ofs_back)
    }

    pub fn pop_nibble(&mut self) -> usize {
        let nibble = self.get(0);
        self.ofs_front += 1;
        debug_assert!(self.ofs_front <= self.ofs_back);
        nibble
    }
}

impl fmt::Debug for NibbleBuf {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_nibbles().fmt(f)
    }
}
impl Into<NibbleBuf> for Nibbles<'_> {
    fn into(self) -> NibbleBuf {
        NibbleBuf {
            bytes: SmallVec::from_slice(&self.bytes),
            ofs_front: self.ofs_front,
            ofs_back: self.ofs_back,
        }
    }
}

impl AsNibbles for NibbleBuf {
    fn as_nibbles(&self) -> Nibbles {
        Nibbles {
            bytes: &self.bytes,
            ofs_front: self.ofs_front,
            ofs_back: self.ofs_back,
        }
    }
}

// The PathSegment is encoded as length in a `u16`, using the most significant
// bit as the bool value.
impl<H> Content<H> for NibbleBuf
where
    H: ByteHash,
{
    fn persist(&mut self, sink: &mut Sink<H>) -> io::Result<()> {
        println!("persisting {} {}", self.ofs_front, self.ofs_back);

        println!("bytes {:?}", self.bytes);

        let byte_range_start = self.ofs_front / 2;
        let byte_range_end = self.ofs_back / 2;

        // normalize offsets
        let mut new_ofs_front: u16 =
            (self.ofs_front - byte_range_start * 2) as u16;
        let mut new_ofs_back: u16 =
            (self.ofs_back - byte_range_start * 2) as u16;

        new_ofs_front.persist(sink)?;
        new_ofs_back.persist(sink)?;

        println!(
            "persisting start {}, end {}",
            byte_range_start, byte_range_end
        );

        println!(
            "writing bytes: {:?}",
            &self.bytes[byte_range_start..byte_range_end]
        );

        sink.write_all(&self.bytes[byte_range_start..=byte_range_end])
    }

    fn restore(source: &mut Source<H>) -> io::Result<Self> {
        let ofs_front = u16::restore(source)? as usize;
        let ofs_back = u16::restore(source)? as usize;

        println!("restoring {} {}", ofs_front, ofs_back);

        let byte_len = ofs_back / 2 + 1;

        println!("restoring bytes: {}", byte_len);

        let mut smallvec = SmallVec::with_capacity(byte_len);
        for _ in 0..byte_len {
            smallvec.push(u8::restore(source)?)
        }

        Ok(NibbleBuf {
            bytes: smallvec,
            ofs_front,
            ofs_back,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use kelvin::{Blake2b, Store};

    #[test]
    fn path_segment_encoding() {
        let store = Store::<Blake2b>::volatile().unwrap();

        for len in 0..32 {
            let mut smallvec = SmallVec::new();
            for i in 0..len {
                smallvec.push(i as u8);

                let mut a = NibbleBuf {
                    bytes: smallvec.clone(),
                    ofs_front: 0,
                    ofs_back: i / 2,
                };

                let mut b = NibbleBuf {
                    bytes: smallvec.clone(),
                    ofs_front: 1,
                    ofs_back: i / 2 + 1,
                };

                let mut c = NibbleBuf {
                    bytes: smallvec.clone(),
                    ofs_front: 0,
                    ofs_back: i / 2,
                };

                let mut d = NibbleBuf {
                    bytes: smallvec.clone(),
                    ofs_front: 1,
                    ofs_back: i / 2 + 1,
                };

                let snap_a = store.persist(&mut a).unwrap();
                let a2 = store.restore(&snap_a).unwrap();
                assert_eq!(a, a2);

                let snap_b = store.persist(&mut b).unwrap();
                let b2 = store.restore(&snap_b).unwrap();
                assert_eq!(b, b2);

                let snap_c = store.persist(&mut c).unwrap();
                let c2 = store.restore(&snap_c).unwrap();
                assert_eq!(c, c2);

                let snap_d = store.persist(&mut d).unwrap();
                let d2 = store.restore(&snap_d).unwrap();
                assert_eq!(d, d2);
            }
        }
    }

    #[test]
    fn nibble_encoding() {
        let a = NibbleBuf::new(&[0, 0]);
        for i in 0..4 {
            assert_eq!(a.get(i), 0);
        }
        let a = NibbleBuf::new(&[255, 255]);
        for i in 0..4 {
            assert_eq!(a.get(i), 0xf);
        }

        let a = NibbleBuf::new(&[0xab, 0xcd]);

        assert_eq!(a.get(0), 0xa);
        assert_eq!(a.get(1), 0xb);
        assert_eq!(a.get(2), 0xc);
        assert_eq!(a.get(3), 0xd);
    }

    #[test]
    fn nibble_common() {
        let a = NibbleBuf::new(&[0x01, 0x23, 0x45, 0x67, 0x89]);
        let b = NibbleBuf::new(&[0x01, 0x23, 0x46, 0xff]);

        println!("a: {:?}", a);
        println!("b: {:?}", b);

        let common: NibbleBuf = a.as_nibbles().common_prefix(&b).into();

        assert_eq!(common.len(), 5);

        for i in 0..5 {
            assert_eq!(common.get(i), i);
        }
    }
}
