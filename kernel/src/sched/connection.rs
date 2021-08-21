use crate::uses::*;
use sys_consts::options::MsgOptions;
use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use crate::util::{Futex, FutexGaurd, NLVecMap};
use crate::syscall::SyscallVals;
use super::*;

/*lazy_static!
{
	pub static ref conn_map: ConnectionMap = ConnectionMap::new ();
}

#[derive(Debug)]
pub struct ConnectionMap
{
	// TODO: usze a nlvecset instead of map because connection id has to be stored in Connection data structure anyway
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
	pub fn new_connection (&self, domain: usize) -> usize
	{
		let id = self.next_id.fetch_add (1, Ordering::Relaxed);
		self.cons.insert (id, Futex::new (Connection::new (id, domain)));
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
	id: usize,
	domain: usize,
	endpoints: Vec<Endpoint>,
}

impl Connection
{
	pub const fn new (id: usize, domain: usize) -> Self
	{
		Connection {
			id,
			domain,
			endpoints: Vec::new (),
		}
	}

	pub fn id (&self) -> usize
	{
		self.id
	}

	pub fn domain (&self) -> usize
	{
		self.domain
	}

	pub fn endpoints (&mut self) -> &mut Vec<Endpoint>
	{
		&mut self.endpoints
	}

	pub fn send_message (&mut self, args: &MsgArgs, blocking: bool) -> Result<Registers, SysErr>
	{
		let pid = proc_c ().pid ();
		let plist = proc_list.lock ();

		let mut i = 0;
		while let Some(endpoint) = self.endpoints.get (i)
		{
			if endpoint.pid != pid
			{
				match plist.get (&endpoint.pid)
				{
					Some(process) => {
						if !process.recieve_message (self, endpoint, args)
						{
							self.endpoints.remove (i);
							continue;
						}
					},
					None => {
						self.endpoints.remove (i);
						continue;
					},
				}
			}
			i += 1;
		}

		drop (plist);

		if blocking
		{
			self.await_response ()
		}
		else
		{
			Err(SysErr::Ok)
		}
	}

	fn await_response (&self) -> Result<Registers, SysErr>
	{
		let new_state = ThreadState::Listening(self.id);
		tlist.ensure (new_state);
		thread_c ().block (new_state);
		*thread_c ().rcv_regs ().lock ()
	}

	pub fn insert_endpoint (&mut self, endpoint: Endpoint)
	{
		if let Err(index) = self.endpoint_index (endpoint.pid)
		{
			self.endpoints.insert (index, endpoint);
		}
	}

	/*pub fn get_endpoint (&self, pid: usize) -> Option<&Endpoint>
	{
		match self.endpoint_index (pid)
		{
			Ok(index) => self.endpoints.get (index),
			Err(_) => None,
		}
	}

	pub fn get_endpoint_mut (&mut self, pid: usize) -> Option<&mut Endpoint>
	{
		match self.endpoint_index (pid)
		{
			Ok(index) => self.endpoints.get_mut (index),
			Err(_) => None,
		}
	}

	pub fn remove_endpoint (&mut self, pid: usize) -> Option<Endpoint>
	{
		match self.endpoint_index (pid)
		{
			Ok(index) => Some(self.endpoints.remove (index)),
			Err(_) => None,
		}
	}*/

	fn endpoint_index (&self, pid: usize) -> Result<usize, usize>
	{
		self.endpoints.binary_search_by (|endpoint| endpoint.pid.cmp (&pid))
	}
}

#[derive(Debug, Clone, Copy)]
pub struct Endpoint
{
	pid: usize,
	tid: usize,
}

impl Endpoint
{
	pub const fn new (pid: usize, tid: usize) -> Self
	{
		Endpoint {
			pid,
			tid,
		}
	}

	pub fn pid (&self) -> usize
	{
		self.pid
	}

	pub fn tid (&self) -> usize
	{
		self.tid
	}
}

#[derive(Debug, Clone, Copy)]
pub struct MsgArgs
{
	pub options: u32,
	pub sender_pid: usize,
	pub domain: usize,
	pub a1: usize,
	pub a2: usize,
	pub a3: usize,
	pub a4: usize,
	pub a5: usize,
	pub a6: usize,
	pub a7: usize,
	pub a8: usize,
}

// TODO: make this work with more than two endpoints, if that feature isn't removed
pub fn msg (vals: &SyscallVals) -> Result<Registers, SysErr>
{
	// FIXME: find where to set rip
	let options = MsgOptions::from_bits_truncate (vals.options);
	let blocking = options.contains (MsgOptions::BLOCK);

	let target_pid = vals.a1;
	let domain = vals.a2;
	let args = MsgArgs {
		options: SysErr::Ok.num () as u32,
		sender_pid: proc_c ().pid (),
		domain,
		a1: vals.a3,
		a2: vals.a4,
		a3: vals.a5,
		a4: vals.a6,
		a5: vals.a7,
		a6: vals.a8,
		a7: vals.a9,
		a8: vals.a10,
	};

	if target_pid == proc_c ().pid ()
	{
		return Err(SysErr::InvlId);
	}

	if options.contains (MsgOptions::REPLY)
	{
		match thread_c ().conn_data ().conn_id
		{
			Some(conn_id) => {
				let mut connection = conn_map.get_connection (conn_id).unwrap ();
				connection.send_message (&args, blocking)
			},
			None => Err(SysErr::InvlArgs),
		}
	}
	else
	{
		let conn_id = conn_map.new_connection (domain);
		let mut connection = conn_map.get_connection (conn_id).unwrap ();
		connection.insert_endpoint (Endpoint::new (proc_c ().pid (), thread_c ().tid ()));

		proc_list.lock ().get (&target_pid).ok_or (SysErr::InvlId)?
			.add_endpoint (&mut connection)?;

		connection.send_message (&args, blocking)
	}
}*/

pub fn msg (vals: &SyscallVals) -> Result<Registers, SysErr>
{
	let options = MsgOptions::from_bits_truncate (vals.options);
	let blocking = options.contains (MsgOptions::BLOCK);

	let process = proc_c ();

	let cid = vals.a1;
	let connection = match process.connections ().lock ().get_int (cid)
	{
		Some(connection) => connection.clone (),
		None => return Err(SysErr::InvlId),
	};

	let args = MsgArgs {
		options: SysErr::Ok.num () as u32,
		sender_pid: process.pid (),
		domain: connection.domain (),
		a1: vals.a2,
		a2: vals.a3,
		a3: vals.a4,
		a4: vals.a5,
		a5: vals.a6,
		a6: vals.a7,
		a7: vals.a8,
		a8: vals.a9,
	};
	unimplemented! ();
}

#[derive(Debug)]
pub struct ConnectionMap
{
	data: BTreeMap<usize, Arc<Connection>>,
	ext_data: BTreeMap<usize, Arc<Connection>>,
	next_id: usize,
}

impl ConnectionMap
{
	pub fn new () -> Self
	{
		ConnectionMap {
			data: BTreeMap::new (),
			ext_data: BTreeMap::new (),
			next_id: 0,
		}
	}

	// TODO: handle connections being closed
	// returns id of connection in this process
	pub fn insert (&mut self, connection: Arc<Connection>) -> usize
	{
		let id = self.next_id;
		self.next_id += 1;
		self.data.insert (id, connection);
		id
	}

	// assocaiates an incoming connection id from another process with a connection id in this process
	pub fn assoc (&mut self, conn_id: usize, ext_conn_id: usize)
	{
		if let Some(connection) = self.data.get (&conn_id)
		{
			self.ext_data.insert (ext_conn_id, connection.clone ());
		}
	}

	/*pub fn remove (&mut self, id: usize) -> Option<Arc<Connection>>
	{
		self.data.remove (&id)
	}*/

	pub fn get_int (&self, conn_id: usize) -> Option<&Arc<Connection>>
	{
		self.data.get (&conn_id)
	}

	pub fn get_ext (&self, conn_id: usize) -> Option<&Arc<Connection>>
	{
		self.ext_data.get (&conn_id)
	}
}

#[derive(Debug)]
pub struct Connection
{
	domain: usize,
	pids: usize,
	pidr: usize,
	init_handler: Option<DomainHandler>,
	wating_thread: Option<MemOwner<Thread>>,
}

impl Connection
{
	pub fn new (domain: usize, handler: DomainHandler, pids: usize) -> Arc<Self>
	{
		Arc::new (Connection {
			domain,
			pids: pids,
			pidr: handler.pid (),
			init_handler: Some(handler),
			wating_thread: None,
		})
	}

	pub fn domain (&self) -> usize
	{
		self.domain
	}
}

#[derive(Debug, Clone, Copy)]
pub struct MsgArgs
{
	pub options: u32,
	pub sender_pid: usize,
	pub domain: usize,
	pub a1: usize,
	pub a2: usize,
	pub a3: usize,
	pub a4: usize,
	pub a5: usize,
	pub a6: usize,
	pub a7: usize,
	pub a8: usize,
}
