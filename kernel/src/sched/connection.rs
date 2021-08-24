use crate::uses::*;
use sys_consts::options::MsgOptions;
use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use crate::util::{Futex, FutexGaurd, NLVecMap};
use crate::syscall::SyscallVals;
use super::*;

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

	pub fn remove (&mut self, conn_id: usize) -> Option<Arc<Connection>>
	{
		Some(self.data.remove (self.get_index (conn_id).ok ()?).conn)
	}

	pub fn get_int (&self, conn_id: usize) -> Option<&Arc<Connection>>
	{
		self.data.get (self.get_index (conn_id).ok ()?).map (|cme| &cme.conn)
	}

	pub fn get_ext (&self, cpid: ConnPid) -> Option<&Arc<Connection>>
	{
		for cme in self.data.iter ()
		{
			if cme.other_cpid () == cpid
			{
				return Some(&cme.conn);
			}
		}
		None
	}
}

#[derive(Debug)]
struct ConnInner
{
	init_handler: Option<DomainHandler>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
		assert_eq! (args.domain, self.domain);

		let process = proc_c ();

		let other_cpid = self.other (process.pid ());
		let other_process = proc_get (other_cpid.pid ()).ok_or (SysErr::MsgTerm)?;

		if let None = other_process.connections ().lock ().get_int (other_cpid.conn_id ())
		{
			return Err(SysErr::InvlId);
		}

		let mut inner = self.data.lock ();
		// handle sending message
		let thread_list = tlist.lock ();
		let thread = unsafe { thread_list[ThreadState::Listening(self.cpids)].get (0).map (|ptr| ptr.unbound ()) };
		drop (thread_list);

		match thread
		{
			Some(thread) => {
				thread.msg_rcv (args);
	
				let mut thread_list = tlist.lock ();
				Thread::move_to (thread, ThreadState::Waiting(thread_c ().tuid ()), Some(&mut thread_list), None).unwrap ();
			},
			None => {
				let handler = match inner.init_handler
				{
					Some(handler) => {
						inner.init_handler = None;
						handler
					},
					None => *other_process.domains ().lock ().get (self.domain).ok_or (SysErr::MsgUnreach)?,
				};
		
				match handler.options ().block_mode
				{
					BlockMode::NonBlocking => {
						let mut regs = Registers::from_msg_args (args);
						regs.rip = handler.rip ();
						other_process.new_thread_regs (regs, Some(format! ("domain_handler_{}", self.domain))).or (Err(SysErr::MsgUnreach))?;
					},
					BlockMode::Blocking(tid) => {
						match other_process.get_thread (tid)
						{
							Some(thread) => {
								thread.push_conn_state (args)?;
							},
							None => {
								other_process.domains ().lock ().remove (other_process.pid (), Some(self.domain));
								return Err(SysErr::MsgTerm);
							},
						}
					},
				}
			},
		}

		if blocking
		{
			let new_state = ThreadState::Listening(self.cpids);
			tlist.ensure (new_state);
			drop (inner);
			thread_c ().block (new_state);
			*thread_c ().rcv_regs ().lock ()
		}
		else
		{
			Err(SysErr::Ok)
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub struct MsgArgs
{
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
