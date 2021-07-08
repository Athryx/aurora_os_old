use spin::{Mutex};
use core::cmp::{min, max};
use core::sync::atomic::{Ordering, AtomicUsize};
use crate::uses::*;
use crate::util::{LinkedList, Node, MemOwner, UniqueRef, Futex};
use crate::mb2::{BootInfo, MemoryRegionType};
use super::PAGE_SIZE;
use super::PhysRange;
use super::virt_alloc::FrameAllocator;

const MAX_ORDER: usize = 32;
pub const MAX_ZONES: usize = 4;

pub static zm: ZoneManager = ZoneManager::new ();

#[derive(Debug, Clone, Copy)]
pub struct Allocation
{
	ptr: VirtAddr,
	len: usize,
	zindex: usize,
}

impl Allocation
{
	// NOTE: panics if addr is not canonical
	pub fn new (addr: usize, len: usize) -> Self
	{
		Allocation {
			ptr: VirtAddr::new (addr as _),
			len,
			zindex: 0,
		}
	}

	pub fn as_mut_ptr<T> (&mut self) -> *mut T
	{
		self.ptr.as_mut_ptr ()
	}

	pub fn as_ptr<T> (&self) -> *const T
	{
		self.ptr.as_ptr ()
	}

	pub fn as_slice (&self) -> &[u8]
	{
		unsafe { core::slice::from_raw_parts (self.as_ptr (), self.len) }
	}

	pub fn as_mut_slice (&mut self) -> &mut [u8]
	{
		unsafe { core::slice::from_raw_parts_mut (self.as_mut_ptr (), self.len) }
	}

	pub fn as_usize (&self) -> usize
	{
		self.ptr.as_u64 () as usize
	}

	pub fn len (&self) -> usize
	{
		self.len
	}

	pub fn as_phys_zone (&self) -> PhysRange
	{
		PhysRange::new (virt_to_phys (self.ptr), self.len)
	}
}

#[derive(Debug)]
pub struct BuddyAllocator
{
	start: usize,
	meta_start: *mut u8,
	olist: [LinkedList<Node>; MAX_ORDER],
	max_order: usize,
	min_order_size: usize,
	// the number of bits long that min_order_size is
	min_order_bits: usize,
	free_space: usize,
}

impl BuddyAllocator
{
	// NOTE: start and end are aligned to min_order_size's allignment
	pub unsafe fn new (start: VirtAddr, end: VirtAddr, min_order_size: usize) -> Self
	{
		let min_order_size = max (align_up (min_order_size, PAGE_SIZE), PAGE_SIZE);

		let start = start.align_up (min_order_size as u64).as_u64 () as usize;
		let end = end.as_u64 () as usize;

		if end <= start
		{
			panic! ("allocator passed invalid memory region");
		}

		let meta_size = ((end - start) / (8 * min_order_size)) + 1;
		let meta_start = align_down (end - meta_size, min_order_size);

		let meta_startp = meta_start as *mut u8;

		memset (meta_startp, meta_size, 0);

		let mut out = BuddyAllocator {
			start,
			meta_start: meta_startp,
			olist: init_array! (LinkedList<Node>, MAX_ORDER, LinkedList::new ()),
			max_order: 0,
			min_order_size,
			min_order_bits: log2 (min_order_size),
			free_space: meta_start - start,
		};

		out.init_orders ();

		out
	}

	unsafe fn init_orders (&mut self)
	{
		let mut a = self.start;
		let ms = self.meta_start as usize;
		while a < ms
		{
			let len = min (align_of (a), 1 << log2 (ms - a));

			let order = self.get_order (len);
			let node = Node::new (a, len);

			if order > MAX_ORDER
			{
				panic! ("MAX_ORDER for buddy allocator was smaller than order {}", order);
			}

			self.olist[order].push (node);
			if order > self.max_order
			{
				self.max_order = order;
			}

			a += len;
		}
	}

	fn get_order (&self, size: usize) -> usize
	{
		let bits = log2_up (size);
		if bits <= self.min_order_bits
		{
			0
		}
		else
		{
			bits - self.min_order_bits
		}
	}

	// might panic if order is to big
	fn get_order_size (&self, order: usize) -> usize
	{
		1 << (order + self.min_order_bits)
	}

	fn is_alloced (&self, addr: usize) -> bool
	{
		if addr < self.start || addr >= self.meta_start as usize
		{
			return false;
		}

		let i = (addr - self.start) / self.min_order_size;
		let b = unsafe { *self.meta_start.add (i / 8) };
		b & (1 << (i % 8)) > 0
	}

	fn set_is_alloced (&self, addr: usize, alloced: bool)
	{
		if addr < self.start || addr >= self.meta_start as usize
		{
			return;
		}

		let i = (addr - self.start) / self.min_order_size;
		let ptr = unsafe { self.meta_start.add (i / 8) };
		let mut b = unsafe { *ptr };
		if alloced
		{
			b |= 1 << (i % 8);
		}
		else
		{
			b &= !(1 << (i % 8));
		}
		unsafe { *ptr = b; }
	}

	fn split_order (&mut self, order: usize) -> bool
	{
		if order > self.max_order || order == 0
		{
			return false;
		}

		if self.olist[order].len () != 0 || self.split_order (order + 1)
		{
			let node = self.olist[order].pop_front ().unwrap ();
			node.set_size (node.size ().wrapping_shr (1));

			let addr = node.addr () ^ node.size ();
			let node2 = unsafe { Node::new (addr, node.size ()) };

			self.olist[order - 1].push_front (node2);

			self.olist[order - 1].push_front (node);

			return true;
		}

		false
	}

	fn insert_node (&mut self, mut node: MemOwner<Node>)
	{
		let mut order = self.get_order (node.size ());

		loop
		{
			let addr2 = node.addr () ^ node.size ();
			if addr2 < node.addr () || addr2 > self.start || addr2 + node.size () > self.meta_start as usize || self.is_alloced (addr2)
			{
				break;
			}
			let node2 = unsafe { UniqueRef::new ((addr2 as *const Node).as_ref ().unwrap ()) };

			if node.size () != node2.size ()
			{
				break;
			}

			let node2 = self.olist[order].remove_node (node2);

			// make borrow checker happy
			node = if addr2 < node.addr ()
			{
				//node = node2;
				node2
			}
			else
			{
				node
			};

			node.set_size (node.size () << 1);
			order += 1;

			if order == self.max_order
			{
				break;
			}
		}

		self.olist[order].push_front (node);
	}

	fn order_expand_cap (&self, mem: Allocation) -> usize
	{
		let mut out = 0;
		let mut size = mem.len ();
		let addr = mem.as_usize ();

		loop
		{
			let addr2 = addr ^ size;

			if addr2 < addr || (addr2 + size) > self.meta_start as usize || self.is_alloced (addr2)
			{
				break;
			}

			out += 1;
			size <<= 1;
		}

		out
	}

	pub fn contains (&self, mem: Allocation) -> bool
	{
		let addr = mem.as_usize ();
		addr >= self.start && addr + mem.len () <= self.meta_start as usize && align_of (addr) >= self.min_order_size
	}

	// size is in bytes
	pub fn alloc (&mut self, size: usize) -> Option<Allocation>
	{
		if size == 0
		{
			return None
		}

		let order = self.get_order (size);
		if order > self.max_order
		{
			None
		}
		else
		{
			self.oalloc (order)
		}
	}

	pub fn oalloc (&mut self, order: usize) -> Option<Allocation>
	{
		if order > self.max_order
		{
			return None;
		}

		if self.olist[order].len () == 0 && !self.split_order (order + 1)
		{
			return None;
		}

		// list is guarunteed to contain a node
		let node = self.olist[order].pop_front ().unwrap ();
		let out = Allocation::new (node.addr (), node.size ());

		self.set_is_alloced (node.addr (), true);
		self.free_space -= node.size ();

		Some(out)
	}

	pub unsafe fn realloc (&mut self, mem: Allocation, size: usize) -> Option<Allocation>
	{
		if size == 0
		{
			return None
		}

		let order = self.get_order (size);
		if order > self.max_order
		{
			None
		}
		else
		{
			self.orealloc (mem, order)
		}
	}

	// if none is returned, the original allocation is still valid
	pub unsafe fn orealloc (&mut self, mem: Allocation, order: usize) -> Option<Allocation>
	{
		if order > self.max_order
		{
			return None
		}

		let addr = mem.as_usize ();
		let len = mem.len ();
		if addr < self.start || addr + len > self.meta_start as usize
		{
			return None;
		}

		if !self.is_alloced (addr)
		{
			panic! ("memory region {:?} was already freed, could not be realloced", mem);
		}

		let mut old = self.get_order (len);

		if order == old
		{
			Some(mem)
		}
		else if order < old
		{
			let odiff = old - order;
			let mut size = self.get_order_size (old);
			while old > order
			{
				size >>= 1;
				old -= 1;

				// should already have its metadata marked as free
				let node = Node::new (addr ^ size, size);
				self.olist[old].push_front (node);
			}

			self.free_space += self.get_order_size (odiff);
			Some(Allocation::new (addr, size))
		}
		else
		{
			let odiff = order - old;
			if self.order_expand_cap (mem) >= odiff
			{
				// no need to check if each zone we are expanding to is valid
				for order in old..order
				{
					let size2 = self.get_order_size (order);
					let addr2 = addr ^ size2;
					let node = UniqueRef::new ((addr2 as *const Node).as_ref ().unwrap ());
					self.olist[order].remove_node (node);
				}

				self.free_space += self.get_order_size (odiff);
				Some(Allocation::new (addr, self.get_order_size (order)))
			}
			else
			{
				let mut out = self.oalloc (order)?;
				let src_slice = mem.as_slice ();
				out.as_mut_slice ()[..src_slice.len ()].copy_from_slice (src_slice);
				self.dealloc (mem);
				self.free_space += self.get_order_size (odiff);
				Some(out)
			}
		}
	}

	pub unsafe fn dealloc (&mut self, mem: Allocation)
	{
		let addr = mem.as_usize ();

		if addr < self.start || addr + mem.len () > self.meta_start as usize
		{
			return;
		}

		if !self.is_alloced (addr)
		{
			panic! ("double free on memory region {:?}", mem);
		}

		self.set_is_alloced (addr, false);

		let node = Node::new (addr, mem.len ());
		self.free_space += node.size ();

		self.insert_node (node);
	}
}

#[derive(Debug)]
pub struct ZoneManager
{
	zones: RefCell<[Option<Futex<BuddyAllocator>>; MAX_ZONES]>,
	zlen: RefCell<usize>,
	selnum: AtomicUsize,
}

impl ZoneManager
{
	pub const fn new () -> ZoneManager
	{
		ZoneManager {
			//zones: init_array! (Option<Mutex<BuddyAllocator>>, MAX_ZONES, None),
			// TODO: make this automatically follow MAX_ZONES
			zones: RefCell::new ([None, None, None, None]),
			zlen: RefCell::new (0),
			selnum: AtomicUsize::new (0),
		}
	}

	pub unsafe fn init (&self, boot_info: &BootInfo)
	{
		let mut zlen = self.zlen.borrow_mut ();

		for region in &*boot_info.memory_map
		{
			if let MemoryRegionType::Usable(mem) = region
			{
				let start = mem.addr ();
				let end = mem.addr () + mem.size ();

				if *zlen >= MAX_ZONES
				{
					panic! ("MAX_ZONES is not big enough to store an allocator for all the physical memory zones");
				}

				self.zones.borrow_mut ()[*zlen] = Some(Futex::new (BuddyAllocator::new (
					phys_to_virt (start),
					phys_to_virt (end),
					PAGE_SIZE)));

				*zlen += 1;
			}
		}
	}

	fn get_allocer (&self) -> usize
	{
		// I don't really know what the ordering part means, but I think relaxed is good
		let selnum = self.selnum.fetch_add (1, Ordering::Relaxed);
		selnum % *self.zlen.borrow ()
	}

	pub fn alloc (&self, size: usize) -> Option<Allocation>
	{
		let i = self.get_allocer ();
		self.zones.borrow ()[i].as_ref ().unwrap ().lock ()
			.alloc (size).map (|mut a| { a.zindex = i; a } )
	}

	pub fn allocz (&self, size: usize) -> Option<Allocation>
	{
		let mut out = self.alloc (size)?;
		unsafe { memset (out.as_mut_ptr (), out.len (), 0); }
		Some(out)
	}

	pub fn oalloc (&self, order: usize) -> Option<Allocation>
	{
		let i = self.get_allocer ();
		self.zones.borrow ()[i].as_ref ().unwrap ().lock ()
			.oalloc (order).map (|mut a| { a.zindex = i; a } )
	}

	pub fn oallocz (&self, order: usize) -> Option<Allocation>
	{
		let mut out = self.alloc (order)?;
		unsafe { memset (out.as_mut_ptr (), out.len (), 0); }
		Some(out)
	}

	// TODO: support reallocating to a different zone if new size doesn't fit
	pub unsafe fn realloc (&self, mem: Allocation, size: usize) -> Option<Allocation>
	{
		self.zones.borrow ()[mem.zindex].as_ref ().unwrap ().lock ().realloc (mem, size).map (|mut out| {
			out.zindex = mem.zindex;
			out
		})
	}

	pub unsafe fn orealloc (&self, mem: Allocation, order: usize) -> Option<Allocation>
	{
		self.zones.borrow ()[mem.zindex].as_ref ().unwrap ().lock ().orealloc (mem, order).map (|mut out| {
			out.zindex = mem.zindex;
			out
		})
	}

	pub unsafe fn dealloc (&self, mem: Allocation)
	{
		self.zones.borrow ()[mem.zindex].as_ref ().unwrap ().lock ().dealloc (mem);
	}
}

unsafe impl FrameAllocator for ZoneManager
{
	fn alloc_frame (&self) -> Allocation
	{
		self.alloc (PAGE_SIZE).unwrap ()
	}

	unsafe fn dealloc_frame (&self, frame: Allocation)
	{
		let zones = self.zones.borrow ();
		for i in 0..*self.zlen.borrow ()
		{
			let mut z = zones[i].as_ref ().unwrap ().lock ();
			if z.contains (frame)
			{
				z.dealloc (frame);
				break;
			}
		}
	}
}

unsafe impl Send for ZoneManager {}
unsafe impl Sync for ZoneManager {}
