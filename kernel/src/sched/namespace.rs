use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};

use crate::uses::*;
use crate::util::Futex;
use super::*;

lazy_static! {
	// FIXME: name is duplicated in mape an in Namespace data structure
	pub static ref namespace_map: Futex<BTreeMap<String, Weak<Namespace>>> = Futex::new (BTreeMap::new ());
}

#[derive(Debug)]
pub struct Namespace
{
	name: String,
	domains: Futex<DomainMap>,
}

impl Namespace
{
	pub fn new(name: String) -> Arc<Self>
	{
		let mut map = namespace_map.lock();
		match map.get(&name) {
			// destrucor will guarentee weak is removed if dopped, so no panic will occur
			Some(out) => out.upgrade().unwrap(),
			None => {
				let out = Arc::new(Namespace {
					name: name.clone(),
					domains: Futex::new(DomainMap::new()),
				});
				map.insert(name, Arc::downgrade(&out));
				out
			},
		}
	}

	pub fn name(&self) -> &String
	{
		&self.name
	}

	pub fn domains(&self) -> &Futex<DomainMap>
	{
		&self.domains
	}
}

impl Drop for Namespace
{
	fn drop(&mut self)
	{
		namespace_map.lock().remove(&self.name);
	}
}
