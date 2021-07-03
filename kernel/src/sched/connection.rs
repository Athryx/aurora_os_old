use crate::uses::*;
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::util::{Futex, FutexGaurd, NLVecMap};

#[derive(Debug)]
pub struct ConnectionMap
{
	cons: NLVecMap<usize, Futex<Connection>>,
	// FIXME: have a mechanism to reuse ids
	next_id: AtomicUsize,
}

impl ConnectionMap
{
	pub fn new () -> Self
	{
		ConnectionMap {
			cons: NLVecMap::new (),
			next_id: AtomicUsize::new (0),
		}
	}

	// returns connection id
	pub fn new_connection (&self) -> usize
	{
		let id = self.next_id.fetch_add (1, Ordering::Relaxed);
		self.cons.insert (id, Futex::new (Connection::new ()));
		id
	}

	pub fn get_connection (&self, conn_id: usize) -> Option<FutexGaurd<Connection>>
	{
		self.cons.get (&conn_id).map (|futex| futex.lock ())
	}

	pub fn delete_connection (&self, conn_id: usize)
	{
		self.cons.remove (&conn_id);
	}
}

#[derive(Debug)]
pub struct Connection
{
	endpoints: Vec<Endpoint>,
}

impl Connection
{
	pub const fn new () -> Self
	{
		Connection {
			endpoints: Vec::new (),
		}
	}

	pub fn endpoints (&mut self) -> &mut Vec<Endpoint>
	{
		&mut self.endpoints
	}
}

#[derive(Debug, Clone, Copy)]
pub struct Endpoint
{
	pid: usize,
	tid: Option<usize>,
}

impl Endpoint
{
	pub const fn new (pid: usize, tid: Option<usize>) -> Self
	{
		Endpoint {
			pid,
			tid,
		}
	}
}
