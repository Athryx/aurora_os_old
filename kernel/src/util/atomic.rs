use crate::uses::*;
use core::cell::UnsafeCell;
use core::mem::transmute_copy;
use core::fmt::{Debug, Display, Formatter, Error};

// compares mem with cmp_with, if they are equal, it moves result into mem and returns true
// otherwise, it moves mem into cmp_with and returns false
fn compare_swap (mem: &AtomicU128, cmp_with: &AtomicU128, result: &AtomicU128) -> bool
{
	let zf: u8;

	let res_low: u64;
	let res_high: u64;
	let cmp_low: u64;
	let cmp_high: u64;

	unsafe
	{
		asm!("xchg rbx, {2}",
			"lock cmpxchg16b [{0}]",
			"xchg {2}, rbx",
			"setz {1}",
			in(reg) mem,
			out(reg_byte) zf,
			inout(reg) transmute_copy::<UnsafeCell<u64>, u64> (&result.low) => res_low,
			inout("rcx") transmute_copy::<UnsafeCell<u64>, u64> (&result.high) => res_high,
			inout("rax") transmute_copy::<UnsafeCell<u64>, u64> (&cmp_with.low) => cmp_low,
			inout("rdx") transmute_copy::<UnsafeCell<u64>, u64> (&cmp_with.high) => cmp_high,
			);

		*result.low.get () = res_low;
		*result.high.get () = res_high;
		*cmp_with.low.get () = cmp_low;
		*cmp_with.high.get () = cmp_high;
	}

	zf != 0
}

#[repr(C, align(16))]
pub struct AtomicU128
{
	low: UnsafeCell<u64>,
	high: UnsafeCell<u64>
}

impl AtomicU128
{
	pub fn new (num: u128) -> Self
	{
		AtomicU128 {
			low: UnsafeCell::new (num as u64),
			high: UnsafeCell::new (num.wrapping_shr (64) as u64),
		}
	}

	fn num (&self) -> u128
	{
		let low = unsafe { *self.low.get () };
		let high = unsafe { *self.high.get () };

		(low as u128) | ((high as u128) << 64)
	}

	fn copy (&self) -> AtomicU128
	{
		AtomicU128::new (self.num ())
	}

	pub fn load (&self) -> u128
	{
		let cmp_with = Self::new (0);
		let result = Self::new (0);
		compare_swap (self, &cmp_with, &result);
		cmp_with.num ()
	}

	pub fn store (&self, num: u128)
	{
		self.swap (num);
	}

	pub fn swap (&self, num: u128) -> u128
	{
		let num = Self::new (num);
		let res = self.copy ();
		while !compare_swap (self, &res, &num) {}
		res.num ()
	}

	pub fn compare_exchange (&self, current: u128, new: u128) -> Result<u128, u128>
	{
		let current = Self::new (current);
		let new = Self::new (new);

		if compare_swap (self, &current, &new)
		{
			Ok(current.num ())
		}
		else
		{
			Err(current.num ())
		}
	}

	pub fn fetch_update<F> (&self, mut f: F) -> Result<u128, u128>
		where F: FnMut(u128) -> Option<u128>
	{
		let mut val = self.load ();

		while let Some(num) = f (val)
		{
			match self.compare_exchange (val, num)
			{
				Ok(val) => return Ok(val),
				Err(next) => val = next,
			}
		}

		Err(val)
	}
}

impl Debug for AtomicU128
{
	fn fmt (&self, f: &mut Formatter) -> Result<(), Error>
	{
		write! (f, "{}", self.load ()).unwrap ();
		Ok(())
	}
}

impl Display for AtomicU128
{
	fn fmt (&self, f: &mut Formatter) -> Result<(), Error>
	{
		write! (f, "{}", self.load ()).unwrap ();
		Ok(())
	}
}
