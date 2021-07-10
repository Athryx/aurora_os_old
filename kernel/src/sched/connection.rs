use crate::uses::*;
use sys_consts::{options::MsgOptions, MsgErr};
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::util::{Futex, FutexGaurd, NLVecMap};
use crate::syscall::SyscallVals;
use super::{proc_list, tlist, proc_c, thread_c, Registers, ThreadState};

lazy_static!
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

	pub fn send_message (&mut self, args: &MsgArgs, blocking: bool) -> Result<Registers, MsgErr>
	{
		let tid = thread_c ().tid ();
		let pid = proc_c ().pid ();
		let plist = proc_list.lock ();

		let mut i = 0;
		while let Some(endpoint) = self.endpoints.get (i)
		{
			if endpoint.pid != pid || endpoint.tid != tid
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
			Err(MsgErr::NonBlockOk)
		}
	}

	fn await_response (&self) -> Result<Registers, MsgErr>
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
	pub a1: usize,
	pub a2: usize,
	pub a3: usize,
	pub a4: usize,
	pub a5: usize,
	pub a6: usize,
	pub a7: usize,
	pub a8: usize,
}

pub fn msg (vals: &SyscallVals) -> Result<Registers, MsgErr>
{
	// FIXME: find where to set rip
	let options = MsgOptions::from_bits_truncate (vals.options);
	let blocking = options.contains (MsgOptions::BLOCK);

	let target_pid = vals.a1;
	let domain = vals.a2;
	let args = MsgArgs {
		options: MsgErr::Recieve.num () as u32,
		sender_pid: proc_c ().pid (),
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
		return Err(MsgErr::InvlId);
	}

	if options.contains (MsgOptions::REPLY)
	{
		match thread_c ().conn_data ().conn_id
		{
			Some(conn_id) => {
				let mut connection = conn_map.get_connection (conn_id).unwrap ();
				connection.send_message (&args, blocking)
			},
			None => Err(MsgErr::InvlReply),
		}
	}
	else
	{
		let conn_id = conn_map.new_connection (domain);
		let mut connection = conn_map.get_connection (conn_id).unwrap ();
		connection.insert_endpoint (Endpoint::new (proc_c ().pid (), thread_c ().tid ()));

		proc_list.lock ().get (&target_pid).ok_or (MsgErr::InvlId)?
			.add_endpoint (&mut connection);

		connection.send_message (&args, blocking)
	}
}
