use crate::uses::*;
use crate::mem::VirtRange;
use crate::sched::proc_c;

// this trait represents data structures that can be fetched from user controlled memory by syscalls
// safety: because the user controls the memory, the structre shold be defined for all bit patterns
// so mostly structures containing only integers, and no enums
pub unsafe trait UserData: Copy {}

pub unsafe fn fetch_data<T: UserData> (addr: usize) -> Option<T>
{
	if addr == 0
	{
		return None;
	}

	let range = VirtRange::new_unaligned (VirtAddr::try_new (addr as u64).ok ()?, size_of::<T> ());
	proc_c ().addr_space.range_map (range, |data| {
		ptr::read_unaligned (data.as_ptr () as *const T)
	})
}
