use crate::uses::*;
use crate::util::{LinkedList};
use crate::impl_list_node;
use spin::Mutex;
use core::mem;
use core::alloc::{GlobalAlloc, Layout};
use core::cmp::max;
use super::PAGE_SIZE;
use super::phys_alloc::{zm, Allocation};

const INITIAL_HEAP_SIZE: usize = PAGE_SIZE * 8;
const HEAP_INC_SIZE: usize = PAGE_SIZE * 4;
const CHUNK_SIZE: usize = 1 << log2_up_const (mem::size_of::<Node> ());
// TODO: make not use 1 extra space in some scenarios
const INITIAL_CHUNK_SIZE: usize = align_up (mem::size_of::<HeapZone> (), CHUNK_SIZE);

#[global_allocator]
static ALLOCATOR: GlobalAllocator = GlobalAllocator::new ();

#[alloc_error_handler]
fn alloc_error_handler (layout: Layout) -> !
{
	panic! ("allocation error: {:?}", layout);
}

pub fn init ()
{
	ALLOCATOR.init ();
}

#[derive(Debug, Clone, Copy)]
enum ResizeResult
{
	Shrink(usize),
	Remove(usize),
	NoCapacity,
}

#[derive(Debug)]
struct Node
{
	prev: *mut Node,
	next: *mut Node,
	size: usize,
}

impl Node
{
	unsafe fn new<'a> (addr: usize, size: usize) -> &'a mut Self
	{
		let ptr = addr as *mut Node;

		let out = Node {
			prev: 0 as *mut _,
			next: 0 as *mut _,
			size,
		};
		ptr.write (out);

		ptr.as_mut ().unwrap ()
	}

	unsafe fn resize (&mut self, size: usize, align: usize) -> ResizeResult
	{
		if size > self.size
		{
			return ResizeResult::NoCapacity;
		}

		let naddr = align_down (self.addr () + (self.size - size), max (align, CHUNK_SIZE));
		// alignment might make it less
		if naddr < self.addr ()
		{
			return ResizeResult::NoCapacity
		}

		let nsize = naddr - self.addr ();
		if nsize >= CHUNK_SIZE
		{
			self.size = nsize;
			ResizeResult::Shrink(naddr)
		}
		else
		{
			ResizeResult::Remove(naddr)
		}
		// shouldn't need to check for case where allocation only partly covers node, since this should be impossible
	}

	fn merge<'a> (&'a mut self, node: &'a mut Node) -> bool
	{
		if self.addr () + self.size == node.addr ()
		{
			self.size += node.size;
			true
		}
		else
		{
			false
		}
	}
}

impl_list_node! (Node, prev, next);

struct HeapZone
{
	prev: *mut HeapZone,
	next: *mut HeapZone,
	mem: Allocation,
	free_space: usize,
	list: LinkedList<Node>,
}

impl HeapZone
{
	// size is aligned up to page size
	unsafe fn new<'a> (size: usize) -> Option<&'a mut Self>
	{
		let size = align_up (size, PAGE_SIZE);
		let mem = zm.alloc (size)?;
		let size = mem.len ();
		let ptr = mem.as_usize () as *mut HeapZone;

		let mut out = HeapZone {
			prev: 0 as *mut _,
			next: 0 as *mut _,
			mem,
			free_space: size - INITIAL_CHUNK_SIZE,
			list: LinkedList::new (),
		};

		let node = Node::new (mem.as_usize () + INITIAL_CHUNK_SIZE, size - INITIAL_CHUNK_SIZE);
		out.list.push (node);

		ptr.write (out);

		Some(ptr.as_mut ().unwrap ())
	}

	fn empty (&self) -> bool
	{
		self.free_space == 0
	}

	fn contains (&self, addr: usize, size: usize) -> bool
	{
		(addr >= self.addr () + CHUNK_SIZE) && (addr + size <= self.addr () + CHUNK_SIZE + self.mem.len ())
	}

	// must already be removed from list
	// drop wouldn't work, because no function would ever own this value
	unsafe fn delete (&mut self)
	{
		let mem = self.mem;
		mem::forget (self);
		zm.dealloc (mem)
	}

	unsafe fn alloc (&mut self, layout: Layout) -> *mut u8
	{
		let size = layout.size ();
		let align = layout.align ();

		if size > self.free_space
		{
			return null_mut ();
		}

		let mut out = 0;
		// to get around borrow checker
		// node that may need to be removed
		let mut rnode = None;

		for free_zone in self.list.iter_mut ()
		{
			if free_zone.size >= size
			{
				let old_size = free_zone.size;
				match free_zone.resize (size, align)
				{
					ResizeResult::Shrink(addr) => {
						self.free_space -= old_size - free_zone.size;
						out = addr;
						break;
					}
					ResizeResult::Remove(addr) => {
						rnode = Some(free_zone as *mut Node);
						self.free_space -= old_size;
						out = addr;
						break;
					},
					ResizeResult::NoCapacity => continue,
				}
			}
		}

		if let Some(node) = rnode
		{
			// FIXME: find a way to fix ownership issue without doing this
			self.list.remove_node (node.as_mut ().unwrap ());
		}

		out as *mut u8
	}

	// does not chack if ptr is in this zone
	// ptr should be chuk_size aligned
	unsafe fn dealloc (&mut self, ptr: *mut u8, layout: Layout)
	{
		let addr = ptr as usize;
		let size = align_up (layout.size (), max (CHUNK_SIZE, layout.align ()));

		let mut cnode = Node::new (addr, size);
		let (pnode, nnode) = self.get_prev_next_node (addr);

		if let Some(pnode) = pnode
		{
			if pnode.merge (cnode)
			{
				// cnode was never in list, no need to remove
				cnode = pnode;
			}
			else
			{
				self.list.insert_after (cnode, pnode);
			}
		}
		else
		{
			self.list.push_front (cnode);
		}

		if let Some(nnode) = nnode
		{
			if cnode.merge (unbound_mut (nnode))
			{
				self.list.remove_node (nnode);
			}
		}

		self.free_space += size;
	}

	fn get_prev_next_node<'a, 'b> (&'a mut self, addr:usize) -> (Option<&'b mut Node>, Option<&'b mut Node>)
	{
		let mut pnode = None;
		let mut nnode = None;
		for region in unsafe { unbound_mut (self) }.list.iter_mut ()
		{
			if region.addr () > addr
			{
				nnode = Some(region);
				break;
			}
			pnode = Some(region);
		}

		(pnode, nnode)
	}
}

impl_list_node! (HeapZone, prev, next);

struct LinkedListAllocator
{
	list: LinkedList<HeapZone>,
}

impl LinkedListAllocator
{
	fn new () -> LinkedListAllocator
	{
		let node = unsafe { HeapZone::new (INITIAL_HEAP_SIZE)
			.expect ("failed to allocate pages for kernel heap") };
		let mut list = LinkedList::new ();
		list.push (node);

		LinkedListAllocator {
			list,
		}
	}

	unsafe fn alloc (&mut self, layout: Layout) -> *mut u8
	{
		let size = layout.size ();
		let align = layout.align ();

		for z in self.list.iter_mut ()
		{
			if z.free_space >= size
			{
				let ptr = z.alloc (layout);
				if ptr.is_null ()
				{
					continue;
				}
				else
				{
					return ptr;
				}
			}
		}

		// allocate new heapzone because there was no space in any others
		let size_inc = max (HEAP_INC_SIZE, size + max (align, CHUNK_SIZE) + INITIAL_CHUNK_SIZE);
		let zone = match HeapZone::new (size_inc)
		{
			Some(n) => n,
			None => return null_mut (),
		};

		self.list.push (zone);

		// shouldn't fail now
		zone.alloc (layout)
	}

	unsafe fn dealloc (&mut self, ptr: *mut u8, layout: Layout)
	{
		let addr = ptr as usize;
		assert! (align_of (addr) >= CHUNK_SIZE);
		let size = layout.size ();

		for z in self.list.iter_mut ()
		{
			if z.contains (addr, size)
			{
				z.dealloc (ptr, layout);
				return;
			}
		}

		panic! ("invalid pointer passed to dealloc");
	}
}

// TODO: add relloc function
struct GlobalAllocator
{
	allocer: Mutex<Option<LinkedListAllocator>>,
}

impl GlobalAllocator
{
	const fn new () -> GlobalAllocator
	{
		GlobalAllocator {
			allocer: Mutex::new (None),
		}
	}

	fn init (&self)
	{
		*self.allocer.lock () = Some(LinkedListAllocator::new ());
	}
}

unsafe impl GlobalAlloc for GlobalAllocator
{
	unsafe fn alloc (&self, layout: Layout) -> *mut u8
	{
		self.allocer.lock ().as_mut ().unwrap ().alloc (layout)
	}

	unsafe fn dealloc (&self, ptr: *mut u8, layout: Layout)
	{
		self.allocer.lock ().as_mut ().unwrap ().dealloc (ptr, layout)
	}
}

unsafe impl Send for GlobalAllocator {}
unsafe impl Sync for GlobalAllocator {}
