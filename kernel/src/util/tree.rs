use crate::uses::*;
use core::fmt::{self, Formatter, Display};
use crate::util::{MemCell, UniqueRef, UniqueMut};

pub enum ParentType<'a, T>
{
	LeftOf(&'a T),
	RightOf(&'a T),
	Root,
}

// Any nodes inserted into the avl tree must implement this trait
pub unsafe trait TreeNode: Sized
{
	type Key: Ord;

	fn parent (&self) -> *const Self;
	fn set_parent (&self, parent: *const Self);

	fn left (&self) -> *const Self;
	fn set_left (&self, left: *const Self);
	fn right (&self) -> *const Self;
	fn set_right (&self, right: *const Self);

	fn key (&self) -> &Self::Key;
	fn set_key (&mut self, key: Self::Key);

	fn balance (&self) -> i8;
	fn set_balance (&self, balance: i8);

	// sets the left child, and sets the left child's parent if applicable
	fn set_leftp (&self, left: *const Self)
	{
		self.set_left (left);
		unsafe
		{
			left.as_ref ().map (|node| node.set_parent (self.as_ptr ()));
		}
	}

	// sets the left child, and sets the left child's parent if applicable
	fn set_rightp (&self, right: *const Self)
	{
		self.set_right (right);
		unsafe
		{
			right.as_ref ().map (|node| node.set_parent (self.as_ptr ()));
		}
	}

	fn inc_balance (&self, num: i8)
	{
		self.set_balance (self.balance () + num);
	}

	fn as_ptr (&self) -> *mut Self
	{
		self as *const _ as *mut _
	}

	fn is_balanced (&self) -> bool
	{
		self.balance () >= -1 && self.balance () <= 1
	}

	fn child_count (&self) -> u32
	{
		let mut out = 0;

		if !self.left ().is_null ()
		{
			out += 1;
		}

		if !self.right ().is_null ()
		{
			out += 1;
		}

		out
	}

	// TODO: see if swapping key and value is ok
	// swaps 2 nodes, returns some with a pointer if the root node needs to be set
	fn swap (&self, other: &Self) -> Option<*const Self>
	{
		let ptr = self.left ();
		self.set_leftp (other.left ());
		other.set_leftp (ptr);

		let ptr = self.right ();
		self.set_rightp (other.right ());
		other.set_rightp (ptr);

		let mut out = None;

		let ptr = self.parent ();
		let parent = self.parent_type ();
		self.set_parent (other.parent ());

		match parent
		{
			ParentType::LeftOf(node) =>	node.set_left (other.as_ptr ()),
			ParentType::RightOf(node) => node.set_right (other.as_ptr ()),
			ParentType::Root => out = Some(other.as_ptr () as *const _),
		}

		let parent = other.parent_type ();
		other.set_parent (ptr);
		match parent
		{
			ParentType::LeftOf(node) =>	node.set_left (self.as_ptr ()),
			ParentType::RightOf(node) => node.set_right (self.as_ptr ()),
			ParentType::Root => out = Some(self.as_ptr () as *const _),
		}

		other.set_parent (ptr);

		let bf = self.balance ();
		self.set_balance (other.balance ());
		other.set_balance (bf);

		out
	}

	unsafe fn parent_ref (&self) -> &Self
	{
		self.parent ().as_ref ().unwrap ()
	}

	unsafe fn left_ref (&self) -> &Self
	{
		self.left ().as_ref ().unwrap ()
	}

	unsafe fn right_ref (&self) -> &Self
	{
		self.right ().as_ref ().unwrap ()
	}

	fn parent_type (&self) -> ParentType<Self>
	{
		let ptr = self.parent ();
		if ptr.is_null ()
		{
			return ParentType::Root;
		}

		let parent = unsafe { ptr.as_ref ().unwrap () };
		let sptr = self.as_ptr ();

		if sptr as *const Self == parent.left ()
		{
			ParentType::LeftOf(parent)
		}
		else
		{
			ParentType::RightOf(parent)
		}
	}

	fn replace_child (&self, child: *const Self, new: *const Self) -> bool
	{
		if self.left () == child
		{
			self.set_leftp (new);
			true
		}
		else if self.right () == child
		{
			self.set_rightp (new);
			true
		}
		else
		{
			false
		}
	}

	// TODO: make rotations update balance factor
	// panics if there is no left child
	// returns pointer to top of new subtree
	fn rotate_right (&self) -> *const Self
	{
		let left_pointer = self.left ();
		let left_child = unsafe { left_pointer.as_ref ().unwrap () };

		// to stop parent of right child referencing self
		let parent = self.parent ();
		self.set_leftp (left_child.right ());
		left_child.set_rightp (self.as_ptr ());
		left_child.set_parent (parent);

		// parent adjust
		let balance_adjust = if left_child.balance () < 0
		{
			1 - left_child.balance ()
		}
		else
		{
			1
		};
		self.inc_balance (balance_adjust);

		let balance_adjust = if self.balance () > 0
		{
			1 + self.balance ()
		}
		else
		{
			1
		};
		left_child.inc_balance (balance_adjust);

		left_pointer
	}

	// panics if there is no right child
	// returns pointer to top of new subtree
	fn rotate_left (&self) -> *const Self
	{
		let right_pointer = self.right ();
		let right_child = unsafe { right_pointer.as_ref ().unwrap () };

		// to stop parent of right child referencing self
		let parent = self.parent ();
		self.set_rightp (right_child.left ());
		right_child.set_leftp (self.as_ptr ());
		right_child.set_parent (parent);

		// parent adjust
		let balance_adjust = if right_child.balance () > 0
		{
			1 + right_child.balance ()
		}
		else
		{
			1
		};
		self.inc_balance (-balance_adjust);

		let balance_adjust = if self.balance () < 0
		{
			-1 + self.balance ()
		}
		else
		{
			-1
		};
		right_child.inc_balance (balance_adjust);

		right_pointer
	}

	// assumes all children are balanced
	fn rebalance (&self) -> *const Self
	{
		if self.is_balanced ()
		{
			return self.as_ptr ();
		}
		else if self.balance () < -1
		{
			let child = unsafe { self.left_ref () };

			if child.balance () == 1
			{
				self.set_left (child.rotate_left ());
			}

			self.rotate_right ()
		}
		else
		{
			let child = unsafe { self.right_ref () };

			if child.balance () == -1
			{
				self.set_right (child.rotate_right ());
			}

			self.rotate_left ()
		}
	}
}

#[macro_export]
macro_rules! impl_tree_node
{
	($k:ty, $v:ty, $parent:ident, $left:ident, $right:ident, $key:ident, $balance:ident) => {
		unsafe impl $crate::util::TreeNode for $v
		{
			type Key = $k;

			fn parent (&self) -> *const Self
			{
				self.$parent.get ()
			}

			fn set_parent (&self, parent: *const Self)
			{
				self.$parent.set (parent);
			}

			fn left (&self) -> *const Self
			{
				self.$left.get ()
			}

			fn set_left (&self, left: *const Self)
			{
				self.$left.set (left);
			}

			fn right (&self) -> *const Self
			{
				self.$right.get ()
			}

			fn set_right (&self, right: *const Self)
			{
				self.$right.set (right);
			}

			fn key (&self) -> &Self::Key
			{
				&self.$key
			}

			fn set_key (&mut self, key: Self::Key)
			{
				self.$key = key;
			}

			fn balance (&self) -> i8
			{
				self.$balance.get ()
			}

			fn set_balance (&self, balance: i8)
			{
				self.$balance.set (balance);
			}
		}
	};
}

enum SearchResult<T>
{
	Present(*mut T),
	// these two variants represent a node which was not present
	// pointer returned is what their parent would be if they were inserted
	LeftOf(*mut T),
	RightOf(*mut T),
	// the tree is empty, and the value would be the new root
	Root,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BalMode
{
	Add,
	Del,
}

// A non allocating avl tree
#[derive(Debug)]
pub struct AvlTree<K: Ord, V: TreeNode<Key = K>>
{
	root: *const V,
	len: usize,
}

impl<K: Ord, V: TreeNode<Key = K>> AvlTree<K, V>
{
	pub const fn new () -> Self
	{
		AvlTree {
			root: null (),
			len: 0,
		}
	}

	pub fn len (&self) -> usize
	{
		self.len
	}

	// tries to insert value into the tree, if it is already occupied, it returns error with value
	pub fn insert (&mut self, key: K, value: MemCell<V>) -> Result<UniqueMut<V>, MemCell<V>>
	{
		let mut v = value.borrow_mut ();

		v.set_left (null_mut ());
		v.set_right (null_mut ());
		v.set_balance (0);

		match self.search (&key)
		{
			SearchResult::Present(_) => {
				drop (v);
				return Err(value);
			},
			SearchResult::LeftOf(ptr) => {
				let node = unsafe { ptr.as_mut ().unwrap () };

				node.set_leftp (v.as_ptr ());

				v.set_key (key);

				self.rebalance (node, -1, BalMode::Add);
			},
			SearchResult::RightOf(ptr) => {
				let node = unsafe { ptr.as_mut ().unwrap () };

				node.set_rightp (v.as_ptr ());

				v.set_key (key);

				self.rebalance (node, 1, BalMode::Add);
			},
			SearchResult::Root => {
				v.set_parent (null ());

				v.set_key (key);

				self.root = value.ptr ();
			},
		};

		self.len += 1;

		drop (v);
		unsafe
		{
			Ok(UniqueMut::from_ptr (value.ptr_mut ()))
		}
	}

	pub fn remove (&mut self, key: &K) -> Option<MemCell<V>>
	{
		match self.search (key)
		{
			SearchResult::Present(ptr) => {
				self.len -= 1;

				let node = unsafe { ptr.as_ref ().unwrap () };
				if node.left ().is_null () || node.right ().is_null ()
				{
					self.remove_edge_node (node)
				}
				else
				{
					let mut child = unsafe { node.left_ref () };
					while !child.right ().is_null ()
					{
						child = unsafe { child.right_ref () };
					}

					node.swap (child).map (|ptr| self.root = ptr);
					self.remove_edge_node (node)
				}
			},
			_ => return None,
		}
	}

	fn remove_edge_node (&mut self, node: &V) -> Option<MemCell<V>>
	{
		// works if both are null
		let child = if node.right ().is_null ()
		{
			node.left ()
		}
		else if node.left ().is_null ()
		{
			node.right ()
		}
		else
		{
			return None;
		};

		match node.parent_type ()
		{
			ParentType::LeftOf(parent) => {
				parent.set_leftp (child);

				self.rebalance (parent, 1, BalMode::Del);
			},
			ParentType::RightOf(parent) => {
				parent.set_rightp (child);

				self.rebalance (parent, -1, BalMode::Del);
			},
			ParentType::Root => {
				self.root = child;
				unsafe
				{
					child.as_ref ().map (|node| node.set_parent (null ()));
				}
			},
		}

		Some(MemCell::new (node.as_ptr ()))
	}

	pub fn get (&self, key: &K) -> Option<UniqueRef<V>>
	{
		match self.search (key)
		{
			SearchResult::Present(ptr) => unsafe {
				Some(UniqueRef::from_ptr (ptr))
			},
			_ => None,
		}
	}

	pub fn get_mut (&mut self, key: &K) -> Option<UniqueMut<V>>
	{
		match self.search (key)
		{
			SearchResult::Present(ptr) => unsafe {
				Some(UniqueMut::from_ptr (ptr))
			},
			_ => None,
		}
	}

	fn search (&self, key: &K) -> SearchResult<V>
	{
		let mut node = match unsafe { self.root.as_ref () }
		{
			Some(node) => node,
			None => return SearchResult::Root,
		};

		loop
		{
			if key < node.key ()
			{
				node = match unsafe { node.left ().as_ref () }
				{
					Some(node) => node,
					None => return SearchResult::LeftOf(node.as_ptr ()),
				};
			}
			else if key > node.key ()
			{
				node = match unsafe { node.right ().as_ref () }
				{
					Some(node) => node,
					None => return SearchResult::RightOf(node.as_ptr ()),
				};
			}
			else
			{
				return SearchResult::Present(node.as_ptr ());
			}
		}
	}

	fn rebalance (&mut self, mut node: &V, mut bf_change: i8, bal_mode: BalMode)
	{
		loop
		{
			let old_bf = node.balance ();
			let parent = node.parent_type ();

			node.inc_balance (bf_change);
			let ptr = node.rebalance ();
			node = unsafe { ptr.as_ref ().unwrap () };
			let new_bf = node.balance ();

			// FIXME: ugly
			// true if the subtree has grown 1 in height
			let hchange = if bal_mode == BalMode::Add &&
				old_bf == 0 && new_bf.abs () == 1
			{
				1
			}
			else if bal_mode == BalMode::Del &&
				old_bf.abs () == 1 && new_bf == 0
			{
				-1
			}
			else
			{
				0
			};

			node = match parent
			{
				ParentType::LeftOf(node) => {
					node.set_left (ptr);
					if hchange == 1
					{
						bf_change = -1;
					}
					else if hchange == -1
					{
						bf_change = 1;
					}
					else
					{
						return;
					}
					node
				},
				ParentType::RightOf(node) => {
					node.set_right (ptr);
					if hchange == 1
					{
						bf_change = 1;
					}
					else if hchange == -1
					{
						bf_change = -1;
					}
					else
					{
						return;
					}
					node
				},
				ParentType::Root => {
					self.root = ptr;
					return;
				},
			};
		}
	}
}

impl<K: Ord, V: TreeNode<Key = K> + Display> Display for AvlTree<K, V>
{
	fn fmt (&self, f: &mut Formatter<'_>) -> fmt::Result
	{
		unsafe
		{
			self.root.as_ref ().map (|node| self.print_recurse (f, node, 0));
		}

		Ok(())
	}
}

impl<K: Ord, V: TreeNode<Key = K> + Display> AvlTree<K, V>
{
	fn print_recurse (&self, f: &mut Formatter<'_>, node: &V, depth: usize)
	{
		unsafe
		{
			let flag = node.child_count () == 1;
			match node.right ().as_ref ()
			{
				Some(node) => self.print_recurse (f, node, depth + 1),
				None => if flag {
					Self::print_ident (f, depth + 1);
					write! (f, "========\n").unwrap ();
				},
			}

			Self::print_ident (f, depth);
			Display::fmt (node, f).unwrap ();
			write! (f, "\n").unwrap ();

			match node.left ().as_ref ()
			{
				Some(node) => self.print_recurse (f, node, depth + 1),
				None => if flag {
					Self::print_ident (f, depth + 1);
					write! (f, "========\n").unwrap ();
				},
			}
		}
	}

	fn print_ident (f: &mut Formatter<'_>, depth: usize)
	{
		for _ in 0..depth
		{
				write! (f, "\t").unwrap ();
		}
	}
}

// Not sure if this is actully true, because I don't really understand this send/sync differences, but it is required to make it compile
unsafe impl<K: Ord, V: TreeNode<Key = K> + Send> Send for AvlTree<K, V> {}
