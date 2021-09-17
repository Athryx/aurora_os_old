use crate::uses::*;
use core::marker::PhantomData;
use core::mem::size_of_val;
use core::iter::FusedIterator;

pub trait HwaTag {
	type Elem<'a>: core::fmt::Debug;

	// returns the size of the element, including the tag
	fn size(&self) -> usize;
	fn elem<'a>(&'a self) -> Self::Elem<'a>;

	// convinience function to get internal data
	unsafe fn raw_data<T>(&self) -> &T {
		assert!(size_of_val(self) + size_of::<T>() <= self.size());
		let addr = (self as *const Self as *const u8 as usize) + size_of_val(self);
		(addr as *const T).as_ref().unwrap()
	}
}

// hardware array iterator
// iterates over arrays of different sized elements with different type elements
pub struct HwaIter<'a, T: HwaTag> {
	// start address of elements
	addr: usize,
	// end address of elements
	end: usize,
	//  required alignment of elements
	align: usize,
	phantom: PhantomData<&'a T>,
}

impl<T: HwaTag> HwaIter<'_, T> {
	pub unsafe fn from(addr: usize, size: usize) -> Self {
		HwaIter::<T> {
			addr,
			end: addr + size,
			align: 0,
			phantom: PhantomData,
		}
	}

	pub unsafe fn from_align(addr: usize, size: usize, align: usize) -> Self {
		HwaIter::<T> {
			addr,
			end: addr + size,
			align,
			phantom: PhantomData,
		}
	}
}

impl<'a, T: HwaTag> Iterator for HwaIter<'a, T> {
	type Item = T::Elem<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.addr >= self.end {
			None
		} else {
			let tag = unsafe {
				(self.addr as *const T).as_ref().unwrap()
			};

			self.addr += tag.size();
			if self.addr > self.end {
				return None;
			}

			if self.align != 0 {
				self.addr = align_up(self.addr, self.align);
			}

			Some(tag.elem())
		}
	}
}

impl<T: HwaTag> FusedIterator for HwaIter<'_, T> {}
