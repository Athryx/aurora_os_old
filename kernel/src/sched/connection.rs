use crate::uses::*;
use sys_consts::options::MsgOptions;
use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use crate::util::{Futex, FutexGuard, NLVecMap};
use crate::syscall::SyscallVals;
use super::*;

pub fn msg (vals: &SyscallVals) -> Result<Registers, SysErr>
{
	let options = MsgOptions::from_bits_truncate (vals.options);

	let process = proc_c ();

	let cid = vals.a1;
	let connection = match process.connections ().lock ().get_int (cid)
	{
		Some(connection) => connection.clone (),
		None => return Err(SysErr::InvlId),
	};

	let mut args = MsgArgs {
		smem_mask: 0,
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

	connection.send_message (&mut args, options)
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
	pid: usize,
}

impl ConnectionMap
{
	pub fn new (pid: usize) -> Self
	{
		ConnectionMap {
			data: Vec::new (),
			next_id: 0,
			pid,
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
	pub fn insert (&mut self, connection: Arc<Connection>) -> bool
	{
		let sender = connection.is_sender (self.pid);
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

impl Drop for ConnectionMap
{
	fn drop (&mut self)
	{
		while let Some(connection) = self.data.pop ()
		{
			let other_cpid = connection.other_cpid ();
			let other_pid = other_cpid.pid ();
			let other_cid = other_cpid.conn_id ();
			match proc_get (other_pid)
			{
				Some(proc) => {
					proc.connections ().lock ().remove (other_cid);
				},
				None => continue,
			}
		}
	}
}

#[derive(Debug)]
struct SMemMover<'a, 'b>
{
	data: Vec<usize>,
	proc_send: &'a Arc<Process>,
	proc_rcv: &'b Arc<Process>,
	success: bool,
}

impl<'a, 'b> SMemMover<'a, 'b>
{
	// FIXME: technically recieveing process could use connections for a short time even if msg fails
	// this isn't a huge issue though
	fn new (args: &mut MsgArgs, options: MsgOptions, proc_send: &'a Arc<Process>, proc_rcv: &'b Arc<Process>) -> Self
	{
		let mut out = SMemMover {
			data: Vec::new (),
			proc_send,
			proc_rcv,
			success: false,
		};
		out.move_smem (args, options);
		out
	}

	fn confirm_success (&mut self)
	{
		self.success = true;
	}

	fn move_one (&mut self, smid: usize) -> Option<usize>
	{
		let smem = self.proc_send.get_smem (smid)?;
		let smid = self.proc_rcv.insert_smem (smem);
		self.data.push (smid);
		Some(smid)
	}

	fn move_smem (&mut self, args: &mut MsgArgs, options: MsgOptions)
	{
		// FIXME: ugly
		if options.contains (MsgOptions::SMEM1)
		{
			if let Some(new_smid) = self.move_one (args.a1)
			{
				args.a1 = new_smid;
				args.smem_mask |= 1;
			}
		}

		if options.contains (MsgOptions::SMEM2)
		{
			if let Some(new_smid) = self.move_one (args.a2)
			{
				args.a2 = new_smid;
				args.smem_mask |= 1 << 1;
			}
		}

		if options.contains (MsgOptions::SMEM3)
		{
			if let Some(new_smid) = self.move_one (args.a3)
			{
				args.a3 = new_smid;
				args.smem_mask |= 1 << 2;
			}
		}

		if options.contains (MsgOptions::SMEM4)
		{
			if let Some(new_smid) = self.move_one (args.a4)
			{
				args.a4 = new_smid;
				args.smem_mask |= 1 << 3;
			}
		}

		if options.contains (MsgOptions::SMEM5)
		{
			if let Some(new_smid) = self.move_one (args.a5)
			{
				args.a5 = new_smid;
				args.smem_mask |= 1 << 4;
			}
		}

		if options.contains (MsgOptions::SMEM6)
		{
			if let Some(new_smid) = self.move_one (args.a6)
			{
				args.a6 = new_smid;
				args.smem_mask |= 1 << 5;
			}
		}

		if options.contains (MsgOptions::SMEM7)
		{
			if let Some(new_smid) = self.move_one (args.a7)
			{
				args.a7 = new_smid;
				args.smem_mask |= 1 << 6;
			}
		}

		if options.contains (MsgOptions::SMEM8)
		{
			if let Some(new_smid) = self.move_one (args.a8)
			{
				args.a8 = new_smid;
				args.smem_mask |= 1 << 7;
			}
		}
	}
}

impl<'a, 'b> Drop for SMemMover<'a, 'b>
{
	fn drop (&mut self)
	{
		if !self.success
		{
			for smid in self.data.iter ()
			{
				self.proc_rcv.remove_smem (*smid);
			}
		}
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

impl Default for ConnPid
{
	fn default () -> Self
	{
		Self::new (0, 0)
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

	// locks thread_list
	pub fn get_waiting_thread (&self) -> Option<UniqueRef<Thread>>
	{
		unsafe
		{
			tlist.lock ()[ThreadState::Listening(self.cpids)].get (0).map (|ptr| ptr.unbound ())
		}
	}

	pub fn send_message (&self, args: &mut MsgArgs, options: MsgOptions) -> Result<Registers, SysErr>
	{
		assert_eq! (args.domain, self.domain);

		let process = proc_c ();

		let other_cpid = self.other (process.pid ());
		let other_process = proc_get (other_cpid.pid ()).ok_or (SysErr::MsgTerm)?;

		if other_process.connections ().lock ().get_int (other_cpid.conn_id ()).is_none ()
		{
			return Err(SysErr::InvlId);
		}

		let mut inner = self.data.lock ();

		// handle sending message
		match self.get_waiting_thread ()
		{
			Some(thread) => {
				let mut mv = SMemMover::new (args, options, &process, &other_process);
				mv.confirm_success ();
				thread.msg_rcv (args);
	
				let state = ThreadState::Waiting(thread_c ().tuid ());
				tlist.ensure (state);
				let mut thread_list = tlist.lock ();
				Thread::move_to (thread, ThreadState::Waiting(thread_c ().tuid ()), &mut thread_list);
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
						let mut mv = SMemMover::new (args, options, &process, &other_process);
						let mut regs = Registers::from_msg_args (args);
						regs.rip = handler.rip ();
						other_process.new_thread_regs (regs, Some(format! ("domain_handler_{}", self.domain))).or (Err(SysErr::MsgUnreach))?;
						mv.confirm_success ();
					},
					BlockMode::Blocking(tid) => {
						match other_process.get_thread (tid)
						{
							Some(thread) => {
								let mut mv = SMemMover::new (args, options, &process, &other_process);
								thread.push_conn_state (args)?;
								mv.confirm_success ();
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

		if options.contains (MsgOptions::BLOCK)
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

impl Drop for Connection
{
	// FIXME: free memory used in scheduler que to store waiting thread
	fn drop (&mut self)
	{
		if let Some(thread) = self.get_waiting_thread ()
		{
			*thread.rcv_regs ().lock () = Err(SysErr::MsgTerm);

			let mut thread_list = tlist.lock ();
			Thread::move_to (thread, ThreadState::Waiting(thread_c ().tuid ()), &mut thread_list);
		}

		tlist.dealloc_state (ThreadState::Listening(self.cpids));
	}
}

#[derive(Debug, Clone, Copy)]
pub struct MsgArgs
{
	pub smem_mask: u8,
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
