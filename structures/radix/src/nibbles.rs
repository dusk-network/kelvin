// Copyright (c) DUSK NETWORK. All rights reserved.
// Licensed under the MPL 2.0 license. See LICENSE file in the project root for details.

use core::cmp;
use core::mem;

use canonical::Canon;
use canonical_derive::Canon;
use kelvin::tests::arbitrary;

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
        let byte = self.bytes[byte_index as usize];

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

#[derive(Clone, Default, Canon, Debug)]
pub struct NibbleBuf {
    bytes: Vec<u8>,
    ofs_front: u16,
    ofs_back: u16,
}

impl arbitrary::Arbitrary for NibbleBuf {
    fn arbitrary(
        u: &mut arbitrary::Unstructured<'_>,
    ) -> arbitrary::Result<Self> {
        let bytes = Vec::arbitrary(u)?;
        let (mut ofs_front, mut ofs_back);
        if bytes.len() > 0 {
            ofs_front = u16::arbitrary(u)? % (bytes.len() * 2) as u16;
            ofs_back = u16::arbitrary(u)? % (bytes.len() * 2) as u16;
        } else {
            ofs_front = 0;
            ofs_back = 0;
        }

        if ofs_front > ofs_back {
            mem::swap(&mut ofs_front, &mut ofs_back)
        }
        Ok(NibbleBuf {
            bytes,
            ofs_front: ofs_front,
            ofs_back: ofs_back,
        })
    }
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

impl<'a> PartialEq for Nibbles<'a> {
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
    #[cfg(test)]
    fn new(bytes: &[u8]) -> Self {
        let mut vec: Vec<u8> = Vec::with_capacity(bytes.len());
        vec.extend_from_slice(bytes);
        NibbleBuf {
            bytes: vec,
            ofs_front: 0,
            ofs_back: bytes.len() as u16 * 2,
        }
    }

    pub fn get(&self, i: usize) -> usize {
        self.as_nibbles().get(i)
    }

    pub fn len(&self) -> usize {
        (self.ofs_back - self.ofs_front) as usize
    }

    pub fn trim_front(&mut self, by: usize) {
        self.ofs_front += by as u16;
        debug_assert!(self.ofs_front <= self.ofs_back)
    }

    pub fn pop_nibble(&mut self) -> usize {
        let nibble = self.get(0);
        self.ofs_front += 1;
        debug_assert!(self.ofs_front <= self.ofs_back);
        nibble
    }

    pub fn set(&mut self, idx: usize, to: usize) {
        let byte_index = (self.ofs_front + idx as u16) / 2;
        if byte_index >= self.bytes.len() as u16 {
            self.bytes.push(0);
        };
        let byte = &mut self.bytes[byte_index as usize];

        // pick the right nibble from the byte
        // when front + offset is even, we pick the first
        if (idx + self.ofs_front as usize) % 2 == 0 {
            *byte &= 0x0F;
            *byte |= ((to as u8) << 4) & 0xF0;
        } else {
            *byte &= 0xF0;
            *byte |= to as u8 & 0x0F;
        }
    }

    pub fn push(&mut self, nibble: usize) {
        let i = self.len();
        self.ofs_back += 1;
        self.set(i, nibble);
    }

    pub fn append<A: AsNibbles>(&mut self, other: &A) {
        let nibbles = other.as_nibbles();
        for i in 0..nibbles.len() {
            self.push(nibbles.get(i))
        }
    }
}

impl Into<NibbleBuf> for Nibbles<'_> {
    fn into(self) -> NibbleBuf {
        NibbleBuf {
            bytes: self.bytes.into(),
            ofs_front: self.ofs_front as u16,
            ofs_back: self.ofs_back as u16,
        }
    }
}

impl AsNibbles for NibbleBuf {
    fn as_nibbles(&self) -> Nibbles {
        Nibbles {
            bytes: &self.bytes,
            ofs_front: self.ofs_front as usize,
            ofs_back: self.ofs_back as usize,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use canonical_host::MemStore;
    use kelvin::tests::fuzz_content;

    #[test]
    fn fuzz() {
        fuzz_content::<NibbleBuf, MemStore>();
    }

    #[test]
    fn nibble_common() {
        let a = NibbleBuf::new(&[0x01, 0x23, 0x45, 0x67, 0x89]);
        let b = NibbleBuf::new(&[0x01, 0x23, 0x46, 0xff]);

        let common: NibbleBuf = a.as_nibbles().common_prefix(&b).into();

        assert_eq!(common.len(), 5);

        for i in 0..5 {
            assert_eq!(common.get(i), i);
        }
    }

    #[test]
    fn set_nibbles() {
        let mut a = NibbleBuf::new(&[0x00, 0x00]);
        let b = NibbleBuf::new(&[0x12, 0x34]);

        for i in 0..4 {
            a.set(i, i + 1);
        }

        assert_eq!(a, b);
    }
}
