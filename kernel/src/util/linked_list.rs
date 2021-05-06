use core::ops::{Index, IndexMut};
use core::fmt::{self, Formatter, Debug};
use crate::uses::*;

// I think these need to return raw pointers because using up mut refs would violate ownership rules
pub unsafe trait ListNode
{
	fn next_ptr (&self) -> *mut Self;
	fn set_next (&mut self, next: *mut Self);
	fn prev_ptr (&self) -> *mut Self;
	fn set_prev (&mut self, prev: *mut Self);
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

			pub unsafe fn prev<'a, 'b> (&'a self) -> Option<&'b Self>
			{
				self.$prev.as_ref ()
			}

			pub unsafe fn prev_mut<'a, 'b> (&'a self) -> Option<&'b mut Self>
			{
				self.$prev.as_mut ()
			}

			pub unsafe fn next<'a, 'b> (&'a self) -> Option<&'b Self>
			{
				self.$next.as_ref ()
			}

			pub unsafe fn next_mut<'a, 'b> (&'a self) -> Option<&'b mut Self>
			{
				self.$next.as_mut ()
			}
		}
		unsafe impl $crate::util::ListNode for $ty
		{
			fn next_ptr (&self) -> *mut Self
			{
				self.$next
			}
		
			fn set_next (&mut self, next: *mut Self)
			{
				self.$next = next;
			}
		
			fn prev_ptr (&self) -> *mut Self
			{
				self.$prev
			}
		
			fn set_prev (&mut self, prev: *mut Self)
			{
				self.$prev = prev;
			}
		}
	}
}

// a very common type of ListNode
#[derive(Debug)]
pub struct Node
{
	next: *mut Node,
	prev: *mut Node,
	pub size: usize,
}

impl Node
{
	pub unsafe fn new<'a> (addr: usize, size: usize) -> &'a mut Self
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
}

impl_list_node! (Node, prev, next);

// TODO: make this more safe
// TODO: maybe make in into_iter method
// this linked list doesn't require memory allocation, and it doesn't own any of its values
pub struct LinkedList<T: ListNode>
{
	start: *mut T,
	end: *mut T,
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
	pub fn push (&mut self, val: &mut T)
	{
		if self.len == 0
		{
			self.start = val;
			val.set_prev (null_mut ());
			val.set_next (null_mut ());
		}
		else
		{
			unsafe
			{
				self.end.as_mut ().unwrap ().set_next (val);
			}
			val.set_prev (self.end);
			val.set_next (null_mut ());
		}
		self.end = val;
		self.len += 1;
	}

	pub fn pop<'a, 'b> (&'a mut self) -> Option<&'b mut T>
	{
		if self.len == 0
		{
			return None;
		}

		let out;
		unsafe
		{
			out = self.end.as_mut ().unwrap ();
			if self.len > 1
			{
				self.end = out.prev_ptr ();
				self.end.as_mut ().unwrap ().set_next (null_mut ());
			}
		}

		self.len -= 1;
		Some(out)
	}

	pub fn push_front (&mut self, val: &mut T)
	{
		if self.len == 0
		{
			self.end = val;
			val.set_prev (null_mut ());
			val.set_next (null_mut ());
		}
		else
		{
			unsafe
			{
				self.start.as_mut ().unwrap ().set_prev (val);
			}
			val.set_next (self.start);
			val.set_prev (null_mut ());
		}
		self.start = val;
		self.len += 1;
	}

	pub fn pop_front<'a, 'b> (&'a mut self) -> Option<&'b mut T>
	{
		if self.len == 0
		{
			return None;
		}

		let out;
		unsafe
		{
			out = self.start.as_mut ().unwrap ();
			if self.len > 1
			{
				self.start = out.next_ptr ();
				self.start.as_mut ().unwrap ().set_prev (null_mut ());
			}
		}

		self.len -= 1;
		Some(out)
	}

	pub fn insert (&mut self, index: usize, val: &mut T)
	{
		if index > self.len
		{
			return;
		}

		if index == 0
		{
			self.push_front (val);
			return;
		}

		if index == self.len
		{
			self.push (val);
			return;
		}

		let node = unsafe { unbound_mut (self.get_node_mut (index)) };

		self.insert_before (val, node);
	}

	pub fn remove<'a, 'b> (&'a mut self, index: usize) -> Option<&'b mut T>
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

		// so that node has a different lifetime than self
		let node = unsafe { (self.get_node_mut (index) as *mut T).as_mut ().unwrap () };

		self.remove_node (node);

		Some(node)
	}

	pub fn insert_before (&mut self, new_node: &mut T, node: &mut T)
	{
		assert! (self.len != 0);
		self.len += 1;

		if let Some(prev_node) = unsafe { node.prev_ptr ().as_mut () }
		{
			new_node.set_prev (prev_node as *mut _);
			prev_node.set_next (new_node as *mut _);
		}
		else
		{
			self.start = new_node as *mut _;
			new_node.set_prev (null_mut ());
		}

		node.set_prev (new_node as *mut _);
		new_node.set_next (node as *mut _);
	}

	pub fn insert_after (&mut self, new_node: &mut T, node: &mut T)
	{
		assert! (self.len != 0);
		self.len += 1;

		if let Some(next_node) = unsafe { node.next_ptr ().as_mut () }
		{
			new_node.set_next (next_node as *mut _);
			next_node.set_prev (new_node as *mut _);
		}
		else
		{
			self.end = new_node as *mut _;
			new_node.set_next (null_mut ());
		}

		node.set_next (new_node as *mut _);
		new_node.set_prev (node as *mut _);
	}

	// must pass in node that is in this list
	pub fn remove_node (&mut self, node: &mut T)
	{
		let prev = node.prev_ptr ();
		let next = node.next_ptr ();

		if prev == null_mut ()
		{
			self.start = next;
		}
		else
		{
			unsafe { prev.as_mut ().unwrap ().set_next (next); }
		}

		if next == null_mut ()
		{
			self.end = prev;
		}
		else
		{
			unsafe { next.as_mut ().unwrap ().set_prev (prev); }
		}

		self.len -= 1;
	}

	pub fn update_node (&mut self, old: &mut T, new: &mut T)
	{
		if let Some(prev_node) = unsafe { old.prev_ptr ().as_mut () }
		{
			prev_node.set_next (new as *mut _);
			new.set_prev (prev_node as *mut _);
		}
		else
		{
			self.start = new as *mut _;
			new.set_prev (null_mut ());
		}

		if let Some(next_node) = unsafe { old.next_ptr ().as_mut () }
		{
			next_node.set_prev (new as *mut _);
			new.set_next (next_node as *mut _);
		}
		else
		{
			self.end = new as *mut _;
			new.set_next (null_mut ());
		}
	}

	pub fn get (&self, index: usize) -> Option<&T>
	{
		if index >= self.len { None } else { Some(self.get_node (index)) }
	}

	pub fn get_mut (&mut self, index: usize) -> Option<&mut T>
	{
		if index >= self.len { None } else { Some(self.get_node_mut (index)) }
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
	fn get_node (&self, index: usize) -> &T
	{
		if index >= self.len
		{
			panic! ("LinkedList internal error: get_node called with invalid index");
		}

		let mut node;
		if index * 2 > self.len
		{
			unsafe
			{
				node = self.end.as_ref ().unwrap ();

				for _ in 0..(self.len - index - 1)
				{
					node = node.prev_ptr ().as_ref ().unwrap ();
				}
			}
		}
		else
		{
			unsafe
			{
				node = self.start.as_ref ().unwrap ();

				for _ in 0..index
				{
					node = node.next_ptr ().as_ref ().unwrap ();
				}
			}
		}

		node
	}

	// maybe unsafe
	// must call with valid index
	fn get_node_mut (&mut self, index: usize) -> &mut T
	{
		if index >= self.len
		{
			panic! ("LinkedList internal error: get_node called with invalid index");
		}

		let mut node;
		if index * 2 > self.len
		{
			unsafe
			{
				node = self.end.as_mut ().unwrap ();

				for _ in 0..(self.len - index - 1)
				{
					node = node.prev_ptr ().as_mut ().unwrap ();
				}
			}
		}
		else
		{
			unsafe
			{
				node = self.start.as_mut ().unwrap ();

				for _ in 0..index
				{
					node = node.next_ptr ().as_mut ().unwrap ();
				}
			}
		}

		node
	}
}

impl<T: ListNode> Index<usize> for LinkedList<T>
{
	type Output = T;

	fn index (&self, index: usize) -> &Self::Output
	{
		self.get (index).expect ("ListNode: invalid index")
	}
}

impl<T: ListNode> IndexMut<usize> for LinkedList<T>
{
	fn index_mut (&mut self, index: usize) -> &mut Self::Output
	{
		self.get_mut (index).expect ("ListNode: invalid index")
	}
}

impl<'a, T: ListNode> IntoIterator for &'a LinkedList<T>
{
	type Item = &'a T;
	type IntoIter = Iter<'a, T>;

	fn into_iter (self) -> Self::IntoIter
	{
		self.iter ()
	}
}

impl<'a, T: ListNode> IntoIterator for &'a mut LinkedList<T>
{
	type Item = &'a mut T;
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
	start: *mut T,
	end: *mut T,
	len: usize,
	marker: PhantomData<&'a T>,
}

impl<'a, T: ListNode> Iterator for Iter<'a, T>
{
	type Item = &'a T;

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
			Some(out)
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
			Some(out)
		}
	}
}

impl<T: ListNode> ExactSizeIterator for Iter<'_, T> {}
impl<T: ListNode> core::iter::FusedIterator for Iter<'_, T> {}

pub struct IterMut<'a, T: ListNode>
{
	start: *mut T,
	end: *mut T,
	len: usize,
	marker: PhantomData<&'a T>,
}

impl<'a, T: ListNode> Iterator for IterMut<'a, T>
{
	type Item = &'a mut T;

	fn next (&mut self) -> Option<Self::Item>
	{
		if self.len == 0
		{
			None
		}
		else
		{
			let out = unsafe { self.start.as_mut ().unwrap () };
			self.start = out.next_ptr ();
			self.len -= 1;
			Some(out)
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
			let out = unsafe { self.end.as_mut ().unwrap () };
			self.end = out.prev_ptr ();
			self.len -= 1;
			Some(out)
		}
	}
}

impl<T: ListNode> ExactSizeIterator for IterMut<'_, T> {}
impl<T: ListNode> core::iter::FusedIterator for IterMut<'_, T> {}
