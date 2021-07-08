//use core::ops::{Index, IndexMut};
use core::fmt::{self, Formatter, Debug};
use core::sync::atomic::AtomicPtr;
use core::cell::Cell;
use crate::uses::*;
use crate::util::{MemOwner, UniqueRef, UniqueMut, UniquePtr};

// Safety:
// next_ptr must return pointer value previously set by set_next, same for preve_ptr and set_prev
// next_ptr and prev_ptr values must not be modified by implementor
// only LinkedList should modify these values
pub unsafe trait ListNode
{
	fn next_ptr (&self) -> *const Self;
	fn set_next (&self, next: *const Self);
	fn prev_ptr (&self) -> *const Self;
	fn set_prev (&self, prev: *const Self);
}

#[macro_export]
macro_rules! impl_list_node
{
	($ty:ty, $prev:ident, $next:ident) => {
		impl $ty
		{
			pub fn addr (&self) -> usize
			{
				self as *const _ as usize
			}

			// so I don't have to bring trait into scope every time
			fn next_ptr (&self) -> *mut Self
			{
				self.$next.load (core::sync::atomic::Ordering::Acquire)
			}
		
			fn set_next (&self, next: *mut Self)
			{
				self.$next.store (next, core::sync::atomic::Ordering::Release);
			}
		
			fn prev_ptr (&self) -> *mut Self
			{
				self.$prev.load (core::sync::atomic::Ordering::Acquire)
			}
		
			fn set_prev (&self, prev: *mut Self)
			{
				self.$prev.store (prev, core::sync::atomic::Ordering::Release);
			}

			/*pub unsafe fn prev<'a, 'b> (&'a self) -> Option<&'b Self>
			{
				self.prev_ptr ().as_ref ()
			}

			pub unsafe fn prev_mut<'a, 'b> (&'a self) -> Option<&'b mut Self>
			{
				self.prev_ptr ().as_mut ()
			}

			pub unsafe fn next<'a, 'b> (&'a self) -> Option<&'b Self>
			{
				self.next_ptr ().as_ref ()
			}

			pub unsafe fn next_mut<'a, 'b> (&'a self) -> Option<&'b mut Self>
			{
				self.next_ptr ().as_mut ()
			}*/
		}
		unsafe impl $crate::util::ListNode for $ty
		{
			fn next_ptr (&self) -> *const Self
			{
				self.$next.load (core::sync::atomic::Ordering::Acquire) as *const _
			}
		
			fn set_next (&self, next: *const Self)
			{
				self.$next.store (next as *mut _, core::sync::atomic::Ordering::Release);
			}
		
			fn prev_ptr (&self) -> *const Self
			{
				self.$prev.load (core::sync::atomic::Ordering::Acquire) as *const _
			}
		
			fn set_prev (&self, prev: *const Self)
			{
				self.$prev.store (prev as *mut _, core::sync::atomic::Ordering::Release);
			}
		}
	}
}

// a very common type of ListNode
#[derive(Debug)]
pub struct Node
{
	next: AtomicPtr<Node>,
	prev: AtomicPtr<Node>,
	size: Cell<usize>,
}

impl Node
{
	pub unsafe fn new (addr: usize, size: usize) -> MemOwner<Self>
	{
		let ptr = addr as *mut Node;

		let out = Node {
			prev: AtomicPtr::new (null_mut ()),
			next: AtomicPtr::new (null_mut ()),
			size: Cell::new (size),
		};
		ptr.write (out);

		MemOwner::new (ptr)
	}

	pub fn size (&self) -> usize
	{
		self.size.get ()
	}

	pub fn set_size (&self, size: usize)
	{
		self.size.set (size);
	}
}

impl_list_node! (Node, prev, next);

// TODO: maybe make in into_iter method
// this linked list doesn't require memory allocation
pub struct LinkedList<T: ListNode>
{
	start: *const T,
	end: *const T,
	len: usize,
}

impl<T: ListNode> LinkedList<T>
{
	pub const fn new () -> Self
	{
		LinkedList {
			start: 0 as *mut T,
			end: 0 as *mut T,
			len: 0,
		}
	}

	pub fn len (&self) -> usize
	{
		self.len
	}

	// NOTE: first node prev and last store null
	pub fn push (&mut self, val: MemOwner<T>) -> UniqueMut<T>
	{
		if self.len == 0
		{
			self.start = val.ptr ();
			val.set_prev (null_mut ());
			val.set_next (null_mut ());
		}
		else
		{
			unsafe
			{
				self.end.as_ref ().unwrap ().set_next (val.ptr ());
			}
			val.set_prev (self.end);
			val.set_next (null_mut ());
		}
		self.end = val.ptr ();
		self.len += 1;

		unsafe
		{
			UniqueMut::from_ptr (val.ptr_mut ())
		}
	}

	pub fn pop (&mut self) -> Option<MemOwner<T>>
	{
		if self.len == 0
		{
			return None;
		}

		let out;
		unsafe
		{
			out = MemOwner::new (self.end as *mut _);
			let out_ref = self.end.as_ref ().unwrap ();
			if self.len > 1
			{
				self.end = out_ref.prev_ptr ();
				self.end.as_ref ().unwrap ().set_next (null_mut ());
			}
		}

		self.len -= 1;
		Some(out)
	}

	pub fn push_front (&mut self, val: MemOwner<T>) -> UniqueMut<T>
	{
		if self.len == 0
		{
			self.end = val.ptr ();
			val.set_prev (null_mut ());
			val.set_next (null_mut ());
		}
		else
		{
			unsafe
			{
				self.start.as_ref ().unwrap ().set_prev (val.ptr ());
			}
			val.set_next (self.start);
			val.set_prev (null_mut ());
		}
		self.start = val.ptr ();
		self.len += 1;

		unsafe
		{
			UniqueMut::from_ptr (val.ptr_mut ())
		}
	}

	pub fn pop_front (&mut self) -> Option<MemOwner<T>>
	{
		if self.len == 0
		{
			return None;
		}

		let out;
		unsafe
		{
			out = MemOwner::new (self.start as *mut _);
			let out_ref = self.start.as_ref ().unwrap ();
			if self.len > 1
			{
				self.start = out_ref.next_ptr ();
				self.start.as_ref ().unwrap ().set_prev (null_mut ());
			}
		}

		self.len -= 1;
		Some(out)
	}

	pub fn insert (&mut self, index: usize, val: MemOwner<T>) -> Option<UniqueMut<T>>
	{
		if index > self.len
		{
			return None;
		}

		if index == 0
		{
			return Some(self.push_front (val));
		}

		if index == self.len
		{
			return Some(self.push (val));
		}

		let node = unsafe { UniqueRef::new (self.get_node (index)) };

		Some(self.insert_before (val, node))
	}

	pub fn remove (&mut self, index: usize) -> Option<MemOwner<T>>
	{
		if index >= self.len
		{
			return None;
		}

		if index == 0
		{
			return self.pop_front ();
		}

		if index == self.len - 1
		{
			return self.pop ();
		}

		let node = unsafe { UniqueRef::new (self.get_node (index)) };

		Some(self.remove_node (node))
	}

	pub fn insert_before (&mut self, new_node: MemOwner<T>, node: impl UniquePtr<T>) -> UniqueMut<T>
	{
		assert! (self.len != 0);
		self.len += 1;

		let new_ptr = new_node.ptr ();

		if let Some(prev_node) = unsafe { node.prev_ptr ().as_ref () }
		{
			new_node.set_prev (prev_node as *const _);
			prev_node.set_next (new_ptr);
		}
		else
		{
			self.start = new_ptr;
			new_node.set_prev (null_mut ());
		}

		node.set_prev (new_ptr);
		new_node.set_next (node.ptr ());

		unsafe
		{
			UniqueMut::from_ptr (new_node.ptr_mut ())
		}
	}

	pub fn insert_after (&mut self, new_node: MemOwner<T>, node: impl UniquePtr<T>) -> UniqueMut<T>
	{
		assert! (self.len != 0);
		self.len += 1;

		let new_ptr = new_node.ptr ();

		if let Some(next_node) = unsafe { node.next_ptr ().as_ref () }
		{
			new_node.set_next (next_node as *const _);
			next_node.set_prev (new_ptr);
		}
		else
		{
			self.end = new_ptr;
			new_node.set_next (null_mut ());
		}

		node.set_next (new_ptr);
		new_node.set_prev (node.ptr ());

		unsafe
		{
			UniqueMut::from_ptr (new_node.ptr_mut ())
		}
	}

	// must pass in node that is in this list
	pub fn remove_node (&mut self, node: impl UniquePtr<T>) -> MemOwner<T>
	{
		let prev = node.prev_ptr ();
		let next = node.next_ptr ();

		if prev.is_null ()
		{
			self.start = next;
		}
		else
		{
			unsafe { prev.as_ref ().unwrap ().set_next (next); }
		}

		if next.is_null ()
		{
			self.end = prev;
		}
		else
		{
			unsafe { next.as_ref ().unwrap ().set_prev (prev); }
		}

		self.len -= 1;

		unsafe
		{
			MemOwner::new (node.ptr () as *mut T)
		}
	}

	pub fn update_node (&mut self, old: impl UniquePtr<T>, new: MemOwner<T>)
	{
		let new_ptr = new.ptr ();

		if let Some(prev_node) = unsafe { old.prev_ptr ().as_ref () }
		{
			prev_node.set_next (new_ptr);
			new.set_prev (prev_node as *const _);
		}
		else
		{
			self.start = new_ptr;
			new.set_prev (null_mut ());
		}

		if let Some(next_node) = unsafe { old.next_ptr ().as_ref () }
		{
			next_node.set_prev (new_ptr);
			new.set_next (next_node as *const _);
		}
		else
		{
			self.end = new_ptr;
			new.set_next (null ());
		}
	}

	pub fn get (&self, index: usize) -> Option<UniqueRef<T>>
	{
		if index >= self.len { None } else
			{ Some(unsafe { UniqueRef::new (self.get_node (index)) }) }
	}

	pub fn get_mut (&mut self, index: usize) -> Option<UniqueMut<T>>
	{
		if index >= self.len { None } else
			{ Some(unsafe { UniqueMut::new (self.get_node_mut (index)) }) }
	}

	pub fn g (&self, index: usize) -> UniqueRef<T>
	{
		self.get (index).expect ("ListNode: invalid index")
	}

	pub fn gm (&mut self, index: usize) -> UniqueMut<T>
	{
		self.get_mut (index).expect ("ListNode: invalid index")
	}

	pub fn iter (&self) -> Iter<'_, T>
	{
		Iter {
			start: self.start,
			end: self.end,
			len: self.len,
			marker: PhantomData,
		}
	}

	pub fn iter_mut (&mut self) -> IterMut<'_, T>
	{
		IterMut {
			start: self.start,
			end: self.end,
			len: self.len,
			marker: PhantomData,
		}
	}

	// maybe unsafe
	// must call with valid index
	unsafe fn get_node<'a, 'b> (&'a self, index: usize) -> &'b T
	{
		if index >= self.len
		{
			panic! ("LinkedList internal error: get_node called with invalid index");
		}

		let mut node;
		if index * 2 > self.len
		{
			node = self.end.as_ref ().unwrap ();

			for _ in 0..(self.len - index - 1)
			{
				node = node.prev_ptr ().as_ref ().unwrap ();
			}
		}
		else
		{
			node = self.start.as_ref ().unwrap ();

			for _ in 0..index
			{
				node = node.next_ptr ().as_ref ().unwrap ();
			}
		}

		unbound (node)
	}

	// maybe unsafe
	// must call with valid index
	unsafe fn get_node_mut<'a, 'b> (&'a mut self, index: usize) -> &'b mut T
	{
		if index >= self.len
		{
			panic! ("LinkedList internal error: get_node called with invalid index");
		}

		let mut node;
		if index * 2 > self.len
		{
			node = (self.end as *mut T).as_mut ().unwrap ();

			for _ in 0..(self.len - index - 1)
			{
				node = (node.prev_ptr () as *mut T).as_mut ().unwrap ();
			}
		}
		else
		{
			node = (self.start as *mut T).as_mut ().unwrap ();

			for _ in 0..index
			{
				node = (node.next_ptr () as *mut T).as_mut ().unwrap ();
			}
		}

		unbound_mut (node)
	}
}

impl<'a, T: ListNode> IntoIterator for &'a LinkedList<T>
{
	type Item = UniqueRef<'a, T>;
	type IntoIter = Iter<'a, T>;

	fn into_iter (self) -> Self::IntoIter
	{
		self.iter ()
	}
}

impl<'a, T: ListNode> IntoIterator for &'a mut LinkedList<T>
{
	type Item = UniqueMut<'a, T>;
	type IntoIter = IterMut<'a, T>;

	fn into_iter (self) -> Self::IntoIter
	{
		self.iter_mut ()
	}
}

impl<T: ListNode + Debug> Debug for LinkedList<T>
{
	fn fmt (&self, f: &mut Formatter<'_>) -> fmt::Result
	{
		f.debug_list ().entries (self).finish ().unwrap ();
		Ok(())
	}
}

unsafe impl<T: ListNode> Send for LinkedList<T> {}

// NOTE: it is safe to deallocate nodes returned from Iter and IterMut
pub struct Iter<'a, T: ListNode>
{
	start: *const T,
	end: *const T,
	len: usize,
	marker: PhantomData<&'a T>,
}

impl<'a, T: ListNode> Iterator for Iter<'a, T>
{
	type Item = UniqueRef<'a, T>;

	fn next (&mut self) -> Option<Self::Item>
	{
		if self.len == 0
		{
			None
		}
		else
		{
			let out = unsafe { self.start.as_ref ().unwrap () };
			self.start = out.next_ptr ();
			self.len -= 1;
			Some(UniqueRef::new (out))
		}
	}

	fn size_hint (&self) -> (usize, Option<usize>)
	{
		(self.len, Some(self.len))
	}

	fn last (mut self) -> Option<Self::Item>
	{
		self.next_back ()
	}
}

impl<'a, T: ListNode> DoubleEndedIterator for Iter<'a, T>
{
	fn next_back (&mut self) -> Option<Self::Item>
	{
		if self.len == 0
		{
			None
		}
		else
		{
			let out = unsafe { self.end.as_ref ().unwrap () };
			self.end = out.prev_ptr ();
			self.len -= 1;
			Some(UniqueRef::new (out))
		}
	}
}

impl<T: ListNode> ExactSizeIterator for Iter<'_, T> {}
impl<T: ListNode> core::iter::FusedIterator for Iter<'_, T> {}

pub struct IterMut<'a, T: ListNode>
{
	start: *const T,
	end: *const T,
	len: usize,
	marker: PhantomData<&'a T>,
}

impl<'a, T: ListNode> Iterator for IterMut<'a, T>
{
	type Item = UniqueMut<'a, T>;

	fn next (&mut self) -> Option<Self::Item>
	{
		if self.len == 0
		{
			None
		}
		else
		{
			let out = unsafe { (self.start as *mut T).as_mut ().unwrap () };
			self.start = out.next_ptr ();
			self.len -= 1;
			Some(UniqueMut::new (out))
		}
	}

	fn size_hint (&self) -> (usize, Option<usize>)
	{
		(self.len, Some(self.len))
	}

	fn last (mut self) -> Option<Self::Item>
	{
		self.next_back ()
	}
}

impl<'a, T: ListNode> DoubleEndedIterator for IterMut<'a, T>
{
	fn next_back (&mut self) -> Option<Self::Item>
	{
		if self.len == 0
		{
			None
		}
		else
		{
			let out = unsafe { (self.end as *mut T).as_mut ().unwrap () };
			self.end = out.prev_ptr ();
			self.len -= 1;
			Some(UniqueMut::new (out))
		}
	}
}

impl<T: ListNode> ExactSizeIterator for IterMut<'_, T> {}
impl<T: ListNode> core::iter::FusedIterator for IterMut<'_, T> {}
