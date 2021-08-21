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

	connection.send_message (&args, blocking)
}

#[derive(Debug)]
struct ConnMapEntry
{
	sender: bool,
	conn: Arc<Connection>,
}

impl ConnMapEntry
{
	pub fn this_cpid (&self) -> ConnPid
	{
		self.conn.cpid (self.sender)
	}

	pub fn other_cpid (&self) -> ConnPid
	{
		self.conn.cpid (!self.sender)
	}
}

#[derive(Debug)]
pub struct ConnectionMap
{
	data: Vec<ConnMapEntry>,
	next_id: usize,
}

impl ConnectionMap
{
	pub fn new () -> Self
	{
		ConnectionMap {
			data: Vec::new (),
			next_id: 0,
		}
	}

	fn get_index (&self, conn_id: usize) -> Result<usize, usize>
	{
		self.data.binary_search_by (|probe| probe.this_cpid ().pid ().cmp (&conn_id))
	}

	pub fn next_id (&mut self) -> usize
	{
		let out = self.next_id;
		self.next_id += 1;
		out
	}

	// TODO: handle connections being closed
	// returns true if connection inserted into map
	// connection is connection to insert, pid is pid that this connection map is part of, in order to know which is the internal and extarnal ids
	pub fn insert (&mut self, connection: Arc<Connection>, pid: usize) -> bool
	{
		let sender = connection.is_sender (pid);
		match self.get_index (connection.cpid (sender).conn_id ())
		{
			Ok(_) => false,
			Err(index) => {
				let input = ConnMapEntry {
					sender,
					conn: connection,
				};
				self.data.insert (index, input);
				true
			},
		}
	}

	/*pub fn remove (&mut self, id: usize) -> Option<Arc<Connection>>
	{
		self.data.remove (&id)
	}*/

	pub fn get_int (&self, conn_id: usize) -> Option<&Arc<Connection>>
	{
		self.data.get (self.get_index (conn_id).ok ()?).map (|cme| &cme.conn)
	}

	pub fn get_ext (&self, conn_id: usize) -> Option<&Arc<Connection>>
	{
		for cpid in self.data.iter ()
		{
			if cpid.other_cpid ().conn_id () == conn_id
			{
				return Some(&cpid.conn);
			}
		}
		None
	}
}

#[derive(Debug)]
struct ConnInner
{
	init_handler: Option<DomainHandler>,
	wating_thread: Option<MemOwner<Thread>>,
}

// FIXME: this is an awful name for this structure, and so are all the methods named pid that return this
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnPid
{
	pid: usize,
	conn_id: usize,
}

impl ConnPid
{
	pub fn new (pid: usize, conn_id: usize) -> Self
	{
		ConnPid {
			pid,
			conn_id,
		}
	}

	pub fn pid (&self) -> usize
	{
		self.pid
	}

	pub fn conn_id (&self) -> usize
	{
		self.conn_id
	}
}

#[derive(Debug)]
pub struct Connection
{
	domain: usize,
	cpids: ConnPid,
	cpidr: ConnPid,
	data: Futex<ConnInner>,
}

impl Connection
{
	pub fn new (domain: usize, handler: DomainHandler, cpids: ConnPid, cpidr: ConnPid) -> Arc<Self>
	{
		assert_eq! (handler.pid (), cpidr.pid ());
		Arc::new (Connection {
			domain,
			cpids,
			cpidr,
			data: Futex::new (ConnInner {
				init_handler: Some(handler),
				wating_thread: None,
			}),
		})
	}

	pub fn domain (&self) -> usize
	{
		self.domain
	}

	pub fn this (&self, pid: usize) -> ConnPid
	{
		if pid == self.cpids.pid ()
		{
			self.cpids
		}
		else if pid == self.cpidr.pid ()
		{
			self.cpidr
		}
		else
		{
			panic! ("process is not part of connection it is messaging on");
		}
	}

	pub fn other (&self, pid: usize) -> ConnPid
	{
		if pid == self.cpids.pid ()
		{
			self.cpidr
		}
		else if pid == self.cpidr.pid ()
		{
			self.cpids
		}
		else
		{
			panic! ("process is not part of connection it is messaging on");
		}
	}

	pub fn cpid (&self, sender: bool) -> ConnPid
	{
		if sender
		{
			self.cpids
		}
		else
		{
			self.cpidr
		}
	}

	pub fn is_sender (&self, pid: usize) -> bool
	{
		pid == self.cpids.pid ()
	}

	pub fn send_message (&self, args: &MsgArgs, blocking: bool) -> Result<Registers, SysErr>
	{
		assert! (args.domain == self.domain);

		let process = proc_c ();
		let other_pid = self.other (process.pid ());
		let plock = proc_list.lock ();
		let other_process = plock.get (&other_pid.pid ()).ok_or (SysErr::MsgTerm)?.clone ();

		unimplemented! ();
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
