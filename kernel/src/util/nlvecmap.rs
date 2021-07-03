use crate::uses::*;
use super::{NLVec, to_heap, from_heap};

#[derive(Debug)]
struct MapNode<K, V>
{
	key: K,
	value: V,
}

impl<K: Ord, V> MapNode<K, V>
{
	fn new (key: K, value: V) -> Self
	{
		MapNode {
			key,
			value,
		}
	}

	fn heap (key: K, value: V) -> *mut Self
	{
		to_heap (Self::new (key, value))
	}
}

#[derive(Debug)]
pub struct NLVecMap<K, V> (NLVec<MapNode<K, V>>);

impl<K: Ord + Clone, V> NLVecMap<K, V>
{
	pub fn new () -> Self
	{
		NLVecMap(NLVec::new ())
	}

	pub fn len (&self) -> usize
	{
		self.0.len ()
	}

	pub fn get (&self, key: &K) -> Option<&V>
	{
		self.0.write (|vec| {
			match Self::search (vec, key)
			{
				Ok(index) => {
					unsafe
					{
						Some(&vec[index].as_ref ().unwrap ().value)
					}
				},
				Err(_) => None,
			}
		})
	}

	pub fn insert (&self, key: K, value: V) -> Option<V>
	{
		let node = MapNode::heap (key.clone (), value);
		self.0.write (|vec| {
			match Self::search (vec, &key)
			{
				Ok(index) => {
					let out = vec.remove (index);
					vec.insert (index, node);
					Some(out)
				},
				Err(index) => {
					vec.insert (index, node);
					None
				},
			}
		}).map (|ptr| unsafe { from_heap (ptr).value })
	}

	pub fn remove (&self, key: &K) -> Option<V>
	{
		self.0.write (|vec| {
			match Self::search (vec, &key)
			{
				Ok(index) => Some(vec[index]),
				Err(_) => None,
			}
		}).map (|ptr| unsafe { from_heap (ptr).value })
	}

	// if key is contained in the map, Ok(index of element) is returned
	// else, Err(index where element should go) is returned
	fn search (vec: &Vec<*const MapNode<K, V>>, key: &K) -> Result<usize, usize>
	{
		let mut start = 0;
		let mut end = vec.len ();
		
		loop
		{
			let i = (start + end) / 2;
			let elem = unsafe { &vec[i].as_ref ().unwrap ().key };

			if key < elem
			{
				end = i;
			}
			else if key > elem
			{
				start = i;
			}
			else
			{
				return Ok(i);
			}

			if start == end
			{
				if key > elem
				{
					return Err(i + 1);
				}
				else
				{
					return Err(i);
				}
			}
		}
	}
}
