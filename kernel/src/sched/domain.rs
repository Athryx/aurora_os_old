use crate::uses::*;
use alloc::collections::BTreeMap;
use crate::util::{Futex, optnac};

lazy_static!
{
	pub static ref global_domain_map: Futex<BTreeMap<String, DomainMap>> = Futex::new (BTreeMap::new ());
}

#[derive(Debug, Clone, Copy)]
pub struct DomainHandler
{
	pid: usize,
	rip: usize,
	options: HandlerOptions,
}

impl DomainHandler
{
	pub const fn new (rip: usize, pid: usize, options: HandlerOptions) -> Self
	{
		DomainHandler {
			pid,
			rip,
			options,
		}
	}

	pub fn rip (&self) -> usize
	{
		self.rip
	}

	pub fn pid (&self) -> usize
	{
		self.pid
	}

	pub fn options (&self) -> &HandlerOptions
	{
		&self.options
	}
}

#[derive(Debug, Clone, Copy)]
pub enum BlockMode
{
	// a new thread will be spawned to handler the incoming message
	NonBlocking,
	// the thread with tid usize will block and handler the incoming message
	Blocking(usize),
}

#[derive(Debug, Clone, Copy)]
pub struct HandlerOptions
{
	// If this is true, will block the 
	pub block_mode: BlockMode,
	pub public: bool,
}

impl HandlerOptions
{
	pub fn new (block_mode: BlockMode, public: bool) -> Self
	{
		HandlerOptions {
			block_mode,
			public,
		}
	}
}

#[derive(Debug)]
pub struct DomainMap
{
	// function to call if domain is called and no handler is registered for it
	default_handler: Option<DomainHandler>,
	domains: BTreeMap<usize, DomainHandler>,
}

impl DomainMap
{
	pub const fn new () -> Self
	{
		DomainMap {
			default_handler: None,
			domains: BTreeMap::new (),
		}
	}

	pub fn default_handler (&self) -> Option<DomainHandler>
	{
		self.default_handler
	}

	// returns true if succesfull
	pub fn set_default_handler (&mut self, pid_act: usize, new: Option<DomainHandler>) -> bool
	{
		if optnac (self.default_handler, |h| h.pid == pid_act)
		{
			self.default_handler = new;
			true
		}
		else
		{
			false
		}
	}

	// if domain is none, will set the default handler
	// returns the old handler if one was registered
	pub fn register (&mut self, pid_act: usize, domain: Option<usize>, handler: DomainHandler) -> bool
	{
		match domain
		{
			Some(domain) =>	if optnac (self.domains.get (&domain), |h| h.pid == pid_act) 
			{
				self.domains.insert (domain, handler);
				true
			}
			else
			{
				false
			},
			None => self.set_default_handler (pid_act, Some(handler)),
		}
	}

	// removes specified domain handler, returning the old one if it existed
	pub fn remove (&mut self, pid_act: usize, domain: Option<usize>) -> bool
	{
		match domain
		{
			Some(domain) =>	if optnac (self.domains.get (&domain), |h| h.pid == pid_act) 
			{
				self.domains.remove (&domain);
				true
			}
			else
			{
				false
			},
			None => self.set_default_handler (pid_act, None),
		}
	}

	pub fn get (&self, domain: usize) -> Option<&DomainHandler>
	{
		self.domains.get (&domain).or ((&self.default_handler).as_ref ())
	}
}
